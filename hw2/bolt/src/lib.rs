extern crate serde;
extern crate rmp_serde as rmps;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};

pub mod message_kind {
    pub const REQUEST_LISTING: &str = "LIST";
    pub const RESPONSE_LISTING: &str = "FILES";
    pub const REQUEST_FETCH: &str = "FETCH";
    pub const STREAM_CHUNK: &str = "DATA";
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
struct FileListingResponse {
    files: Vec<FileInfo>,
}

#[cfg(test)]
mod tests {
    use crate::FileInfo;
    use rmps::{Deserializer, Serializer};
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    #[test]
    fn it_works() {
        let mut buf = Vec::new();
        let expected = FileInfo {
            name: "/foo.txt".into(),
            size: 42,
        };

        expected.serialize(&mut Serializer::new(&mut buf)).unwrap();

        let reader = Cursor::new(buf);
        let actual = rmps::from_read::<_, FileInfo>(reader).unwrap();
        assert_eq!(expected, actual);
    }
}
