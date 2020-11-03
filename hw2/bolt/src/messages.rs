use crate::message_kind;
use serde::{Deserialize, Serialize};
use std::io::Read;
use serde::de::DeserializeOwned;
use derive_more::From;

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

trait RequestMessage { }
trait ResponseMessage { }

#[derive(From, Debug)]
pub enum RequestBody {
    Fetch(FileFetchRequest),
    Listing(FileListingRequest),
    AddNode(DhtAddNodeRequest),
    RemoveNode(DhtRemoveNodeRequest),
}

pub trait RmpFromRead
    where Self: MessageKindTagged + DeserializeOwned + Sized
{
    type Error;
    fn from_read<R, C>(reader: R) -> Result<C, Self::Error>
        where R: Read,
              C: From<Self>,
              <Self as RmpFromRead>::Error: From<rmps::decode::Error>
    {
        let x = rmps::from_read::<R, Self>(reader)?;
        Ok(x.into())
    }
}

impl<S> RmpFromRead for S
    where S: MessageKindTagged + for<'de> serde::Deserialize<'de>
{
    type Error = rmp_serde::decode::Error;
}

#[derive(From, Debug)]
pub enum ResponseBody {
    Chunk(FileChunkResponse),
    Listing(FileListingResponse),
    AddedNode(DhtAddNodeResponse),
    RemovedNode(DhtRemoveNodeResponse),
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileFetchRequest {
    pub name: String,
}

impl RequestMessage for FileFetchRequest { }
impl MessageKindTagged for FileFetchRequest {
    fn kind(&self) -> &'static str { message_kind::REQUEST_FETCH }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileChunkResponse {
    pub path: String,
    pub offset: u64,
    pub size: u64,

    #[serde(with = "serde_bytes")]
    pub buffer: Vec<u8>,
}

impl ResponseMessage for FileChunkResponse { }
impl<'a> MessageKindTagged for FileChunkResponse {
    fn kind(&self) -> &'static str { message_kind::RESPONSE_CHUNK }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingRequest {
    dummy: ()
}

impl RequestMessage for FileListingRequest { }
impl MessageKindTagged for FileListingRequest {
    fn kind(&self) -> &'static str { message_kind::REQUEST_LISTING }
}

impl FileListingRequest {
    pub fn new() -> FileListingRequest {
        FileListingRequest { dummy: () }
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct DhtAddNodeRequest {
    pub files: Vec<FileInfo>,
}

impl MessageKindTagged for DhtAddNodeRequest {
    fn kind(&self) -> &'static str {
        message_kind::REQUEST_ADD_NODE
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct DhtAddNodeResponse {
    dummy: (),
}

impl ResponseMessage for DhtAddNodeResponse { }
impl MessageKindTagged for DhtAddNodeResponse {
    fn kind(&self) -> &'static str {
        message_kind::RESPONSE_ADD_NODE
    }
}


#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct DhtRemoveNodeRequest {
    dummy: (),
}

impl RequestMessage for DhtRemoveNodeRequest { }
impl MessageKindTagged for DhtRemoveNodeRequest {
    fn kind(&self) -> &'static str {
        message_kind::REQUEST_REMOVE_NODE
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct DhtRemoveNodeResponse {
    dummy: (),
}

impl ResponseMessage for DhtRemoveNodeResponse { }
impl MessageKindTagged for DhtRemoveNodeResponse {
    fn kind(&self) -> &'static str {
        message_kind::RESPONSE_REMOVE_NODE
    }
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FileListingResponse {
    pub files: Vec<FileInfo>,
}

impl ResponseMessage for FileListingResponse { }
impl MessageKindTagged for FileListingResponse {
    fn kind(&self) -> &'static str {
        message_kind::RESPONSE_LISTING
    }
}

impl<'a> MessageKindTagged for ResponseBody {
    fn kind(&self) -> &'static str {
        match self {
            ResponseBody::Chunk(x) => x.kind(),
            ResponseBody::Listing(x) => x.kind(),
            ResponseBody::AddedNode(x) => x.kind(),
            ResponseBody::RemovedNode(x) => x.kind(),
        }
    }
}

pub trait MessageKindTagged {
    fn kind(&self) -> &'static str;
}


