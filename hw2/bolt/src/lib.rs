extern crate serde;
extern crate rmp_serde as rmps;
extern crate tokio_util;

use std::io::{Cursor};

use serde::{Deserialize, Serialize};
use bytes::{BytesMut};
use rmps::{Serializer};
use rmps::decode::Error;
use tokio_util::codec::Decoder;
use serde::export::PhantomData;
use tokio::prelude::io::AsyncWriteExt;
use async_trait::async_trait;

pub mod message_kind {
    pub const REQUEST_LISTING: &str = "LIST";
    pub const RESPONSE_LISTING: &str = "FILES";
    pub const REQUEST_FETCH: &str = "FETCH";
    pub const RESPONSE_CHUNK: &str = "DATA";
}

pub trait MessageKindTagged {
    fn kind(&self) -> &'static str;
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct MessageHeader {
    pub kind: String,
    pub length: u32,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub size: Option<u64>,
}

impl FileInfo {
    pub fn new(name: String, size: Option<u64>) -> FileInfo {
        FileInfo { name, size }
    }
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

#[async_trait]
pub trait MessageEncoder {
    type Message: MessageKindTagged + Serialize;
    async fn write_to<'a, S: AsyncWriteExt + Unpin + Send>(self, stream: S) -> Result<(), Box<dyn std::error::Error>>;
}

#[async_trait]
impl<M> MessageEncoder for M
    where M: MessageKindTagged + Serialize + Send + Sized
{
    type Message = M;
    async fn write_to<'a, S>(self, mut stream: S) -> Result<(), Box<dyn std::error::Error>>
        where S: AsyncWriteExt + Unpin + Send
    {
        let mut message_buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut message_buf))?;

        let header = MessageHeader {
            kind: self.kind().into(),
            length: message_buf.len() as u32,
        };
        let mut header_buf = Vec::new();
        header.serialize(&mut Serializer::new(&mut header_buf))?;

        stream.write_all(&header_buf).await?;
        stream.write_all(&message_buf).await?;

        Ok(())
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

impl From<FileChunkResponse> for ResponseBody {
    fn from(x: FileChunkResponse) -> Self {
        ResponseBody::Chunk(x)
    }
}

impl From<FileListingResponse> for ResponseBody {
    fn from(x: FileListingResponse) -> Self {
        ResponseBody::Listing(x)
    }
}

impl serde::Serialize for ResponseBody {
    fn serialize<S>(&self, serializer: S) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
        where S: serde::Serializer
    {
        match self {
            ResponseBody::Chunk(x) => x.serialize(serializer),
            ResponseBody::Listing(x) => x.serialize(serializer),
        }
    }
}

impl MessageKindTagged for ResponseBody {
    fn kind(&self) -> &'static str {
        match self {
            ResponseBody::Chunk(x) => x.kind(),
            ResponseBody::Listing(x) => x.kind(),
        }
    }
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
            message_kind::RESPONSE_LISTING => {
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

impl MessageKindTagged for FileFetchRequest {
    fn kind(&self) -> &'static str { message_kind::REQUEST_FETCH }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileChunkResponse {
    path: String,
    offset: u64,
    size: u64,
    buffer: Vec<u8>
}

impl MessageKindTagged for FileChunkResponse {
    fn kind(&self) -> &'static str { message_kind::RESPONSE_CHUNK }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingRequest {
    dummy: ()
}

impl MessageKindTagged for FileListingRequest {
    fn kind(&self) -> &'static str { message_kind::REQUEST_LISTING }
}

impl FileListingRequest {
    pub fn new() -> FileListingRequest {
        FileListingRequest { dummy: () }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingResponse {
    pub files: Vec<FileInfo>,
}

impl MessageKindTagged for FileListingResponse {
    fn kind(&self) -> &'static str {
        message_kind::RESPONSE_LISTING
    }
}

#[cfg(test)]
mod tests {
    use crate::FileInfo;
    use rmps::Serializer;
    use serde::Serialize;
    use std::io::Cursor;

    #[test]
    fn it_works() {
        let mut message_buf = Vec::new();
        let expected = FileInfo {
            name: "/foo.txt".into(),
            size: Some(42),
        };

        expected.serialize(&mut Serializer::new(&mut message_buf)).unwrap();

        let reader = Cursor::new(message_buf);
        let actual = rmps::from_read::<_, FileInfo>(reader).unwrap();
        assert_eq!(expected, actual);
    }
}
