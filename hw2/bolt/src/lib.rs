extern crate serde;
extern crate rmp_serde as rmps;
extern crate tokio_util;

use std::collections::HashMap;
use std::io::{Bytes, Cursor};

use serde::{Deserialize, Serialize};
use bytes::{Buf, BytesMut};
use bytes::buf::BufExt;
use rmps::{Deserializer, Serializer};
use rmps::decode::Error;
use tokio_util::codec::Decoder;
use serde::export::PhantomData;
use std::fmt::Debug;

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

pub trait MessageDecoder {
    type Message;
    type Error: std::error::Error + 'static;

    fn read_from(buf: BytesMut, meta: &MessageHeaderDecoded) -> Result<Self::Message, Self::Error>;
}

impl MessageDecoder for RequestBody {
    type Message = Self;
    type Error = rmps::decode::Error;
    fn read_from(buf: BytesMut, meta: &MessageHeaderDecoded) -> Result<Self::Message, Self::Error> {
        let mut reader = Cursor::new(buf);
        reader.set_position(meta.header_len as u64);
        match meta.header.kind.as_str() {
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


#[derive(PartialEq)]
pub struct MessageHeaderDecoded {
    pub header: MessageHeader,
    pub header_len: u32,
}

impl MessageHeaderDecoded {
    pub fn data_len(&self) -> u32 {
        self.header.length
    }

    pub fn buf_target(&self) -> u32 {
        self.header_len + self.data_len()
    }
}


pub struct MessageCodec<'a, M: MessageDecoder> {
    header: &'a MessageHeaderDecoded,
    decoder: PhantomData<M>,
}

impl<M: MessageDecoder> MessageCodec<'_, M> {
    pub fn new(header: &MessageHeaderDecoded) -> MessageCodec<'_, M> {
        MessageCodec { header, decoder: Default::default() }
    }
}

impl<'a, M: MessageDecoder> Decoder for MessageCodec<'a, M>
    where <M as MessageDecoder>::Error: std::convert::From<std::io::Error>
{
    type Item = M::Message;
    type Error = M::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        M::read_from(src.to_owned(), self.header).map(Some)
    }
}

#[derive(Debug)]
pub enum ResponseBody {
    Chunk(FileChunkResponse),
    Listing(FileListingResponse)
}

impl MessageDecoder for ResponseBody {
    type Message = Self;
    type Error = rmps::decode::Error;

    fn read_from(buf: BytesMut, meta: &MessageHeaderDecoded) -> Result<Self::Message, Self::Error> {
        let mut reader = Cursor::new(buf);
        reader.set_position(meta.header_len as u64);
        match meta.header.kind.as_str() {
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
