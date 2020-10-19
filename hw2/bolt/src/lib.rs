extern crate serde;
extern crate rmp_serde as rmps;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};
use std::io::{Bytes, Cursor};
use bytes::{Buf, BytesMut};
use rmps::decode::Error;
use bytes::buf::BufExt;

pub mod message_kind {
    pub const REQUEST_LISTING: &str = "LIST";
    pub const RESPONSE_LISTING: &str = "FILES";
    pub const REQUEST_FETCH: &str = "FETCH";
    pub const RESPONSE_CHUNK: &str = "DATA";
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct MessageHeader {
    pub kind: String,
    pub length: u32,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileInfo {
    name: String,
    size: u64,
}

#[derive(Debug)]
pub enum RequestBody {
    Fetch(FileFetchRequest),
    Listing(FileListingRequest)
}

impl RequestBody {
    pub fn read_from(buf: BytesMut, header: &MessageHeader, header_len: u32) -> Result<Self, rmps::decode::Error> {
        let mut reader = Cursor::new(buf);
        reader.set_position(header_len as u64);
        match header.kind.as_str() {
            message_kind::REQUEST_FETCH => {
                let x = rmps::from_read::<_, FileFetchRequest>(reader)?;
                Ok(RequestBody::Fetch(x))
            },
            message_kind::REQUEST_LISTING => {
                let x = rmps::from_read::<_, FileListingRequest>(reader)?;
                Ok(RequestBody::Listing(x))
            }
            _ => Err(Error::OutOfRange),
        }
    }
}

#[derive(Debug)]
pub enum ResponseBody {
    Chunk(FileChunkResponse),
    Listing(FileListingResponse)
}

impl ResponseBody {
    pub fn read_from(buf: BytesMut, header: &MessageHeader, header_len: u32) -> Result<Self, rmps::decode::Error> {
        let mut reader = Cursor::new(buf);
        reader.set_position(header_len as u64);
        match header.kind.as_str() {
            message_kind::RESPONSE_CHUNK => {
                let x = rmps::from_read::<_, FileChunkResponse>(reader)?;
                Ok(ResponseBody::Chunk(x))
            },
            message_kind::REQUEST_LISTING => {
                let x = rmps::from_read::<_, FileListingResponse>(reader)?;
                Ok(ResponseBody::Listing(x))
            }
            _ => Err(Error::OutOfRange),
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileFetchRequest {
    name: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileChunkResponse {
    path: String,
    offset: u64,
    size: u64,
    buffer: Vec<u8>
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingRequest {
    dummy: ()
}

impl FileListingRequest {
    pub fn new() -> FileListingRequest {
        FileListingRequest { dummy: () }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingResponse {
    files: Vec<FileInfo>,
}

#[cfg(test)]
mod tests {
    use crate::{FileInfo, MessageHeader, message_kind};
    use rmps::{Deserializer, Serializer};
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    #[test]
    fn it_works() {
        let mut message_buf = Vec::new();
        let expected = FileInfo {
            name: "/foo.txt".into(),
            size: 42,
        };

        expected.serialize(&mut Serializer::new(&mut message_buf)).unwrap();

        let reader = Cursor::new(message_buf);
        let actual = rmps::from_read::<_, FileInfo>(reader).unwrap();
        assert_eq!(expected, actual);
    }
}
