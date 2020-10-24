use std::path::{PathBuf, Path};
use std::sync::Arc;
use std::ops::DerefMut;

use tokio::prelude::*;
use tokio::task;
use tokio::fs::File;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use walkdir::{WalkDir, DirEntry};

use bolt::buffer::BufferContext;
use bolt::codec::{read_next, MessageEncoder, MessageReader};
use bolt::messages::{RequestBody, ResponseBody, FileInfo};
use bolt::messages;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::time::{delay_for, Duration};

pub async fn run(listener: &mut TcpListener, root_directory: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (mut socket_raw, peer_address) = listener.accept().await?;
        let (socket_read, socket_write) = socket_raw.into_split();
        let socket_read = Arc::new(Mutex::new(socket_read));
        let socket_write = Arc::new(Mutex::new(socket_write));

        let root_directory = root_directory.clone();
        tokio::spawn(async move {
            let root_directory = root_directory;
            let mut ctx = BufferContext::new(32 * 1024);
            loop {
                let mut lock = socket_read.lock().await;
                let result = lock.deref_mut().read_next::<RequestBody>(&mut ctx, peer_address.into()).await;
                drop(lock);

                println!("[server] received from client {:?}", result);

                if let Ok(req) = result {
                    if let RequestBody::Fetch(fetch) = req {
                        handle_fetch(fetch, socket_write.clone(), root_directory.clone());
                    } else {
                        let copy = root_directory.clone();
                        let result = task::spawn_blocking(|| {
                            let path = copy;
                            handle_sync_request(req, path.as_path()).ok()
                        }).await;

                        println!("[server] handler -> {:?}", result);
                        if let Ok(Some(response)) = result {
                            let mut socket_lock = socket_write.lock().await;
                            let send_result = response.write_to(socket_lock.deref_mut()).await;
                            drop(socket_lock);
                            println!("[server] sent response! {:?}", send_result);
                        } else {
                            eprintln!("[server] bad message from response handler: {:?}", result);
                        }
                    }
                } else {
                    println!("[server] shutdown client handler");
                    break;
                }
            }
        });
    }
}

fn handle_fetch(fetch: messages::FileFetchRequest, socket_ref: Arc<Mutex<OwnedWriteHalf>>, root_directory: PathBuf) -> task::JoinHandle<()> {
    tokio::spawn(async move {
        let fetch = fetch;
        let root_directory = root_directory;
        println!("[server] root = '{}'", &root_directory.to_string_lossy());

        let raw_path = Path::new(&fetch.name);
        let filename = if raw_path.is_absolute() {
            raw_path.strip_prefix("/").unwrap()
        } else { raw_path };
        let absolute_path = root_directory.join(filename);
        println!("[server] fetch request for '{}'", &absolute_path.to_string_lossy());
        let mut file = File::open(absolute_path.as_path()).await;
        if let Err(e) = file {
            eprintln!("[server] failed to open file '{}': {}", &absolute_path.to_string_lossy(), e);
            return;
        }
        let mut file = file.unwrap();

        let total_size = file.metadata().await.unwrap().len();
        let mut buf = [0u8; 32 * 1024];
        let mut off = 0;
        loop {
            let num_read = file.read(&mut buf).await.unwrap();
            if num_read == 0 {
                break;
            }

            let message = messages::FileChunkResponse {
                path: fetch.name.to_string(),
                offset: off,
                size: total_size,
                buffer: buf[0..num_read].to_vec(),
            };

            {
                let mut socket_lock = socket_ref.lock().await;
                message.write_to(socket_lock.deref_mut()).await.unwrap();
            }

            off += num_read as u64;
            let progress = ((off as f64 / total_size as f64) * 100.0);
            // println!("[server] sent '{}' ({}/{} bytes | {}%)", fetch.name, off, total_size, progress);
        }

        println!("[server] finished fetch");
        {
            let mut socket_lock = socket_ref.lock().await;
            socket_lock.deref_mut().flush().await.unwrap();
        }

        delay_for(Duration::from_millis(1000)).await;
    })
}

pub fn handle_sync_request<P: AsRef<Path>>(req: RequestBody, dir: P) -> Result<ResponseBody, Box<dyn std::error::Error>> {
    match req {
        RequestBody::Listing(_) => {
            let entires: Vec<FileInfo> = WalkDir::new(&dir).into_iter()
                .take(100)
                .filter_map(Result::ok)
                .filter(|f: &DirEntry| f.metadata().map_or(false, |meta| meta.is_file()))
                .map(|x: DirEntry| {
                    let name = x.path().strip_prefix(&dir).unwrap().to_string_lossy().to_string();
                    let size = x.metadata().map(|f| f.len()).ok();
                    FileInfo::new(name, size)
                })
                .collect();

            Ok(messages::FileListingResponse { files: entires }.into())
        },
        _ => {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported request").into())
        }
    }
}
