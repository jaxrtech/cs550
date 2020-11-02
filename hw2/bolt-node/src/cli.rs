use std::borrow::{Borrow, BorrowMut};
use std::path::Path;
use std::net::SocketAddr;

use snafu::{ResultExt, Snafu, OptionExt, IntoError, ensure};
use url::Url;
use log::{info, debug, error};

use tokio::prelude::*;
use tokio::fs::File;
use tokio::net::TcpStream;

use bolt::buffer::BufferContext;
use bolt::codec::{MessageEncoder, MessageReader};
use bolt::messages;
use bolt::messages::*;

#[derive(Debug, Snafu)]
enum CommandError {
    #[snafu(display("Bad argument to command: {}", reason))]
    BadArgument { reason: String },

    #[snafu(display("Failed to parse address: {}", source))]
    BadAddressFormat { source: std::net::AddrParseError },

    #[snafu(display("Failed to parse URL: {}", source))]
    BadUrlFormat { source: url::ParseError },

    #[snafu(display("Unable to resolve address: {}", source))]
    AddressResolveFailed { source: std::io::Error },

    #[snafu(display("No addresses were associated with the host"))]
    NoAddressFound,

    #[snafu(display("No file path was specified in the URL"))]
    UrlMissingPath,

    #[snafu(display("Failed to connect to remote host: {}", source))]
    ConnectionError { source: std::io::Error },

    #[snafu(display("Failed to send message to remote host: {}", source))]
    SendError { source: std::io::Error },

    #[snafu(display("Failed to receive message to remote host: {}", source))]
    ReceiveError { source: bolt::codec::MessageReadError<rmps::decode::Error> },

    #[snafu(display("Failed to encode message: {}", source))]
    MessageEncodingError { source: Box<dyn std::error::Error> },

    #[snafu(display("Response was invalid or unexpected: {}", reason))]
    BadResponseError { reason: String },

    #[snafu(display("Unable to write to download file: {}", source))]
    DownloadIoError { source: std::io::Error },
}

async fn cmd_fetch(args: Vec<String>) -> Result<(), CommandError> {
    let url = args.get(0)
        .context(BadArgument { reason: "Expected URL to file to download".to_string() })
        .and_then(|s| Url::parse(s).context(BadUrlFormat))?;

    let addresses = url.socket_addrs(|| Some(8080)).context(AddressResolveFailed)?;
    ensure!(!addresses.is_empty(), NoAddressFound);
    let mut client = TcpStream::connect(addresses.get(0).unwrap()).await
        .context(ConnectionError)?;

    let peer_address = (&client).peer_addr().ok();

    let path = url.path();
    ensure!(!path.is_empty(), UrlMissingPath);

    let filename = Path::new(path).strip_prefix("/").unwrap();
    info!("[client] downloading to '{}'", filename.to_string_lossy());
    let mut output_file = File::create(filename).await
        .context(DownloadIoError)?;

    let req = FileFetchRequest { name: filename.to_string_lossy().to_string() };
    req.write_to(&mut client).await
        .context(MessageEncodingError)?;
    info!("[client] sent request");

    info!("[client] waiting on response...");
    let mut size: Option<u64> = None;
    let mut pos = 0u64;
    let mut ctx = BufferContext::new(20 * 1024);

    while size.map_or(true, |size| pos < size) {
        let resp_any = client.borrow_mut().read_next::<ResponseBody>(&mut ctx, peer_address).await
            .context(ReceiveError)?;

        if let ResponseBody::Chunk(resp) = resp_any {
            if size.is_none() {
                size = Some(resp.size);
            }

            let buffer = resp.buffer;
            // println!("[client] got {} bytes", buffer.len());
            output_file.write_all(buffer.as_slice()).await
                .context(DownloadIoError)?;

            pos += buffer.len() as u64;
            // println!("[client] fetching '{}' ({}/{} bytes)", filename.to_string_lossy(), pos, size.unwrap());
        } else {
            let err = BadResponseError { reason: format!("Expected 'FileChunkResponse' but got '{:?}'", resp_any) };
            return err.fail().map_err(|x| x.into())
        }
    }

    info!("client: done!");
    Ok(())
}

async fn cmd_list(args: Vec<&str>) -> Result<(), CommandError> {
    let mut ctx = BufferContext::new(8 * 1024);

    let address = args.get(0)
        .context(BadArgument { reason: "Expected address with port".to_string() })
        .and_then(|s| s.parse::<SocketAddr>().context(BadAddressFormat))?;

    let mut client = TcpStream::connect(address).await
        .context(ConnectionError)?;

    let peer_address = client.borrow().peer_addr().ok();

    let req = messages::FileListingRequest::new();
    req.write_to(&mut client).await
        .context(MessageEncodingError)?;
    info!("[client] sent request");

    info!("[client] waiting on response...");
    let resp_any = client.borrow_mut().read_next::<ResponseBody>(&mut ctx, peer_address).await
        .context(ReceiveError)?;

    debug!("client got {:?}", resp_any);
    if let ResponseBody::Listing(resp) = resp_any {
        if resp.files.is_empty() {
            println!("(no files available)");
        }
        else {
            let width = resp.files.iter().map(|f| f.name.len()).max().unwrap_or_default();
            for f in resp.files.iter() {
                let formatted_size = f.size.map_or("(unknown size)".into(), |n| format!("{} bytes", n));
                println!("{:width$} | {}", f.name, formatted_size, width = width)
            }
        }
    } else {
        let err = BadResponseError { reason: format!("Expected 'FileListingResponse' but got '{:?}'", resp_any) };
        return err.fail().map_err(|x| x.into())
    }

    Ok(())
}

pub async fn handle_cli_command(line: String) {
    debug!("got '{}'", line);

    let mut parts: Vec<&str> = line.split_whitespace().collect();
    let result = if let Some(command) = parts.get(0) {
        match command.to_lowercase().as_str() {
            "list" => {
                parts.remove(0);
                Some(cmd_list(parts).await)
            },
            "fetch" => {
                parts.remove(0);
                let parts: Vec<String> = parts.iter().map(|x| x.clone().to_string()).collect();
                tokio::spawn(async move { cmd_fetch(parts).await; });
                Some(Ok(()))
            }
            _ => {
                error!("unknown command {}", command);
                None
            }
        }
    } else {
        None
    };

    if let Some(Err(e)) = result {
        error!("{}", e);
    }
}
