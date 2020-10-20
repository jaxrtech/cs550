extern crate serde;
extern crate futures;
extern crate rmp_serde as rmps;

use std::io;
use std::io::prelude::*;
use std::io::{Cursor, ErrorKind};
use std::thread;
use std::net::SocketAddr;
use std::fmt::Debug;
use std::path::Path;

use futures::executor::block_on;
use bytes::BytesMut;
use walkdir::{WalkDir, DirEntry};
use snafu::{ResultExt, Snafu, OptionExt, IntoError};

use tokio::io::AsyncReadExt;
use tokio::{task};
use tokio::prelude::*;
use tokio::runtime::Handle;
use tokio::net::{TcpListener, TcpStream};

use bolt::{MessageHeader, FileListingRequest, ResponseBody, RequestBody, MessageHeaderDecoded, MessageDecoder, FileInfo, FileListingResponse, MessageEncoder};
use tokio::sync::watch;

#[derive(PartialEq)]
enum BufferState {
    Empty,
    WaitHeader,
    WaitData,
    Done,
}

struct BufferContext {
    buffer: BytesMut,
    header_with_target: Option<MessageHeaderDecoded>,
}

impl BufferContext {
    fn new(capacity: usize) -> BufferContext {
        BufferContext {
            buffer: BytesMut::with_capacity(capacity),
            header_with_target: None,
        }
    }

    fn reset(&mut self) {
        self.header_with_target = None;
        self.buffer.clear();
    }

    fn header(&self) -> Option<&MessageHeader> {
        self.header_with_target.as_ref().map(|x| &x.header)
    }

    fn set_header_with_len(&mut self, header: MessageHeader, header_len: u32) {
        self.header_with_target = Some(MessageHeaderDecoded { header, header_len });
    }
    
    fn try_read_header(&mut self) -> Result<(), rmps::decode::Error> {
        let mut cursor = Cursor::new(&self.buffer);
        cursor.set_position(0);

        let parse_result = rmps::from_read::<_, MessageHeader>(&mut cursor);
        let result = match parse_result {
            Ok(header) => {
                let header_len = cursor.position() as u32;
                self.set_header_with_len(header, header_len);
                Ok(())
            }
            Err(e) => Err(e)
        };

        result
    }

    fn get_state(&self) -> BufferState {
        let pos = self.buffer.len() as u32;

        if pos == 0 {
            BufferState::Empty
        }
        else { // pos > 0
            if let Some(target) = &self.header_with_target {
                if pos >= target.buf_target() {
                    BufferState::Done
                }
                else {
                    BufferState::WaitData
                }
            }
            else {
                BufferState::WaitHeader
            }
        }
    }
}

#[derive(Debug, Snafu)]
enum CommandError {
    #[snafu(display("Bad argument to command: {}", reason))]
    BadArgument { reason: String },

    #[snafu(display("Failed to parse address: {}", source))]
    BadAddressFormat { source: std::net::AddrParseError },

    #[snafu(display("Failed to connect to remote host: {}", source))]
    ConnectionError { source: std::io::Error },

    #[snafu(display("Failed to send message to remote host: {}", source))]
    SendError { source: std::io::Error },

    #[snafu(display("Failed to encode message: {}", source))]
    MessageEncodingError { source: Box<dyn std::error::Error> }
}

async fn cmd_list(ctx: &mut BufferContext, args: Vec<&str>) -> Result<(), CommandError> {
    let address = args.get(0)
        .context(BadArgument { reason: "Expected address with port".to_string() })
        .and_then(|s| s.parse::<SocketAddr>().context(BadAddressFormat))?;

    let mut client = TcpStream::connect(address).await
        .context(ConnectionError)?;

    let message = FileListingRequest::new();
    message.write_to(&mut client).await
        .context(MessageEncodingError);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listen_address: &'static str = "127.0.0.1:8080";
    let mut listener = TcpListener::bind(listen_address).await?;
    println!("listening on {}", listen_address);

    let (tx, mut rx) = watch::channel("".to_string());
    let handle = Handle::current();
    thread::spawn(move || {
        print!("> ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        for result in stdin.lock().lines() {
            let line = result.unwrap();
            if !line.is_empty() {
                tx.broadcast(line);
            }

            print!("> ");
            io::stdout().flush().unwrap();
        }
    });

    tokio::spawn(async move {
        let mut ctx = BufferContext::new(32 * 1024);
        while let Some(line) = rx.recv().await {
            if line.is_empty() { continue; }
            handle_cli_command(&mut ctx, handle.clone(), line).await;
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut ctx = BufferContext::new(8 * 1024);
            loop {
                ctx.reset();
                let result = read_next::<RequestBody>(&mut socket, &mut ctx).await;
                println!("got {:?}", result);

                if let Ok(req) = result {
                    let result = task::spawn_blocking(|| {
                        handle_request( req, Box::from(Path::new("."))).ok()
                    }).await;

                    println!("handler -> {:?}", result);
                    if let Ok(Some(response)) = result {
                        response.write_to(&mut socket);
                    }
                }
                else {
                    break;
                }
            }
        });
    }
}

fn handle_request<'a>(req: RequestBody, dir: Box<Path>) -> Result<ResponseBody, Box<dyn std::error::Error>> {
    match req {
        RequestBody::Listing(_) => {
            let entires: Vec<FileInfo> = WalkDir::new(dir).into_iter()
                .take(100)
                .filter_map(Result::ok)
                .map(|x: DirEntry| {
                    let name = x.path().to_string_lossy().to_string();
                    let size = x.metadata().map(|f| f.len()).ok();
                    FileInfo::new(name, size)
                })
                .collect();

            Ok(FileListingResponse { files: entires }.into())
        },
        _ => {
            Err(io::Error::new(ErrorKind::Other, "Unsupported request").into())
        }
    }
}

#[derive(Debug, Snafu)]
enum MessageReadError<M: std::error::Error + 'static> {
    #[snafu(display("Peer disconnected (gracefully): {:?}", peer_addr))]
    PeerDisconnectedGracefully { peer_addr: Option<SocketAddr> },

    #[snafu(display("Failed to read from socket: {}", source))]
    SocketReadError { source: std::io::Error },

    #[snafu(display("Message format error: {:?}", source))]
    BadMessageFormat { source: M }
}

async fn read_next<R: MessageDecoder>(socket: &mut TcpStream, ctx: &mut BufferContext) -> Result<R::Message, MessageReadError<R::Error>> {
    let peer_addr: Option<SocketAddr> = socket.peer_addr().map_or(None, Some);
    let peer_addr_str = peer_addr.map_or("(unknown)".into(), |addr| addr.to_string());

    loop {
        let old_state = ctx.get_state();

        let n = match socket.read_buf(&mut ctx.buffer).await {
            // socket closed
            Ok(n) if n == 0 => {
                println!("[{}] peer disconnected", peer_addr_str);
                (PeerDisconnectedGracefully { peer_addr }).fail()
            }
            Ok(n) => Ok(n),
            Err(e) => {
                let context = SocketReadError;
                Err(context.into_error(e))
            }
        }?;

        println!("[{}] read {} bytes", peer_addr_str, n);

        if old_state == BufferState::Empty {
            ctx.try_read_header();
        }

        // Check if we have read the body at this point
        let new_state = ctx.get_state();
        if new_state == BufferState::Done {
            let target = ctx.header_with_target.as_ref().unwrap();
            let kind = &target.header.kind;
            println!("got data of {} bytes with message of kind {:?}", target.data_len(), kind);

            let message = R::read_from(ctx.buffer.clone(), &target)
                .context(BadMessageFormat)?;
            return Ok(message);
        }
    }
}

async fn handle_cli_command(ctx: &mut BufferContext, handle: Handle, line: String) {
    println!("debug: got '{}'", line);

    let mut parts: Vec<&str> = line.split_whitespace().collect();
    if let Some(command) = parts.get(0) {
        match command.to_lowercase().as_str() {
            "list" => {
                parts.remove(0);
                let result = cmd_list(ctx, parts).await;
                if let Err(e) = result {
                    println!("{}", e);
                }
            }
            _ => println!("unknown command {}", command)
        }
    }
}
