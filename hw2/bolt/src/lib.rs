extern crate serde;
extern crate rmp_serde as rmps;
extern crate tokio_util;
extern crate derive_more;

pub mod messages;
pub mod codec;
pub mod buffer;
pub mod nodes;
pub mod consensus;
mod util;

pub mod message_kind {
    pub const REQUEST_LISTING: &str = "LIST";
    pub const RESPONSE_LISTING: &str = "FILES";

    pub const REQUEST_ADD_NODE: &str = "DHT.ADD";
    pub const RESPONSE_ADD_NODE: &str = "DHT.ADD.OK";

    pub const REQUEST_REMOVE_NODE: &str = "DHT.DEL";
    pub const RESPONSE_REMOVE_NODE: &str = "DHT.DEL.OK";

    pub const REQUEST_FETCH: &str = "FETCH";
    pub const RESPONSE_CHUNK: &str = "DATA";
}

#[cfg(test)]
mod tests {
    use rmps::Serializer;
    use serde::Serialize;
    use std::io::Cursor;
    use crate::messages::FileInfo;

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
