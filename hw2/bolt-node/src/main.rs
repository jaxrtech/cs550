extern crate rmp_serde as rmps;

use std::io;
use std::io::prelude::*;
use std::io::Cursor;

use tokio::prelude::*;
use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use rmps::{Deserializer, Serializer};
use bytes::BytesMut;

use bolt::MessageHeader;
use std::thread;

#[derive(PartialEq)]
enum BufferState {
    Empty,
    WaitHeader,
    GotHeader,
    WaitData,
    Done,
}

#[derive(PartialEq)]
struct BufferTarget {
    header: MessageHeader,
    header_len: u32,
}

impl BufferTarget {
    fn data_len(&self) -> u32 {
        self.header.length
    }

    fn buf_target(&self) -> u32 {
        self.header_len + self.data_len()
    }
}

struct BufferContext {
    buffer: BytesMut,
    header_with_target: Option<BufferTarget>,
}

impl BufferContext {
    fn new(capacity: usize) -> BufferContext {
        BufferContext {
            buffer: BytesMut::with_capacity(capacity),
            header_with_target: None,
        }
    }

    fn header(&self) -> Option<&MessageHeader> {
        self.header_with_target.as_ref().map(|x| &x.header)
    }

    fn set_header_with_len(&mut self, header: MessageHeader, header_len: u32) {
        self.header_with_target = Some(BufferTarget { header, header_len });
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
            Err(err) => {
                Err(err)
            }
        };

        result
    }

    fn get_state(&self) -> (BufferState, u32) {
        let pos = self.buffer.len() as u32;
        let state = if pos == 0 {
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
        };

        (state, pos)
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listen_address: &'static str = "127.0.0.1:8080";
    let mut listener = TcpListener::bind(listen_address).await?;
    println!("listening on {}", listen_address);

    thread::spawn(|| {
        print!("> ");
        io::stdout().flush().unwrap();

        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            println!("debug: got '{}'", line.unwrap());
            print!("> ");
            io::stdout().flush().unwrap();
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut ctx = BufferContext::new(8 * 1024);
            let peer_addr = socket.peer_addr().map_or("(unknown)".into(), |addr| addr.to_string());

            loop {
                let (old_state, old_pos) = ctx.get_state();

                let n = match socket.read_buf(&mut ctx.buffer).await {
                    // socket closed
                    Ok(n) if n == 0 => {
                        println!("[{}] peer disconnected", peer_addr);
                        return;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("[{}] error: failed to read from socket; err = {:?}", peer_addr, e);
                        return;
                    }
                };

                println!("[{}] read {} bytes", peer_addr, n);

                if old_state == BufferState::Empty {
                    ctx.try_read_header();
                }

                // Check if we have read the body at this point
                let (new_state, _) = ctx.get_state();
                if new_state == BufferState::Done {
                    let target = ctx.header_with_target.as_ref().unwrap();
                    println!("got data of {} bytes with message of kind {:?}", target.data_len(), target.header.kind)
                }
            }
        });
    }
}
