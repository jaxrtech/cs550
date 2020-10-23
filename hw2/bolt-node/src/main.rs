mod cli;
mod server;

extern crate serde;
extern crate futures;
extern crate rmp_serde as rmps;

use std::{io, env};
use std::io::prelude::*;
use std::io::{Cursor, ErrorKind, BufReader};
use std::thread;
use std::net::SocketAddr;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::RefCell;
use std::borrow::{BorrowMut, Borrow};
use std::sync::{Arc, RwLock};
use std::ops::DerefMut;

use tokio::prelude::*;
use tokio::runtime::Handle;
use tokio::task;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use futures::executor::block_on;
use bytes::BytesMut;
use walkdir::{WalkDir, DirEntry};
use snafu::{ResultExt, Snafu, OptionExt, IntoError, ensure};
use url::Url;
use clap::Clap;

use bolt::buffer::BufferContext;
use bolt::messages::{RequestBody, FileChunkResponse, ResponseBody};
use bolt::codec::MessageDecoder;

#[derive(Clap)]
#[clap(version = "0.1", author = "Kevin K. <kbknapp@gmail.com>")]
struct Opts {
    /// An alternative port to listen on
    #[clap(short, long, default_value = "8080")]
    port: u16,

    /// Runs the node as the DHT node
    #[clap(long)]
    dht: bool,

    /// The root directory to serve files from
    #[clap(parse(from_os_str))]
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();
    let cwdir = env::current_dir().unwrap();
    let listen_port = opts.port;
    let root_directory = opts.path.unwrap_or(cwdir.clone());
    println!("root = '{}'", &root_directory.to_string_lossy());

    let listen_address: String = format!("127.0.0.1:{}", listen_port);
    let mut listener = TcpListener::bind(&listen_address).await?;
    println!("listening on {}", &listen_address);

    let (tx, mut rx) = watch::channel("".to_string());
    let handle = Handle::current();
    thread::spawn(move || {
        print!("> ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        for result in stdin.lock().lines() {
            let line = result.unwrap();
            if !line.is_empty() {
                tx.broadcast(line).unwrap();
            }

            print!("> ");
            io::stdout().flush().unwrap();
        }
    });

    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if line.is_empty() { continue; }
            cli::handle_cli_command(line.clone()).await;
            println!("handled '{}'", line);
        }
    });

    server::run(&mut listener, root_directory).await
}
