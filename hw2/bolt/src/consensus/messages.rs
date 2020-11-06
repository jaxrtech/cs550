use std::io::Cursor;
use std::borrow::Borrow;
use derive_more::From;
use bytes::{BytesMut, Buf};
use serde::{Deserialize, Serialize};
use crate::consensus::{TermIndex, NodeId};
use crate::codec::{MessageDecoder, MessageHeaderDecoded};
use crate::messages::{MessageKindTagged, RmpFromRead};

pub mod message_kind {
    pub const REQUEST_VOTE: &str = "~ELECTION";
    pub const VOTE_REPLY: &str = "~VOTE";
    pub const HELLO: &str = "~HELLO";
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct HeartbeatMessage {
    pub node_id: NodeId,
}

impl MessageKindTagged for HeartbeatMessage {
    fn kind(&self) -> &'static str {
        message_kind::HELLO
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct RequestVoteArgsMessage {
    pub term: TermIndex,
    pub candidate_id: NodeId,
    pub last_log_index: u32,
    pub last_log_term: TermIndex,
}

impl MessageKindTagged for RequestVoteArgsMessage {
    fn kind(&self) -> &'static str {
        message_kind::REQUEST_VOTE
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct VoteReplyMessage {
    pub term: TermIndex,
    pub vote_graded: bool,
}

impl MessageKindTagged for VoteReplyMessage {
    fn kind(&self) -> &'static str {
        message_kind::VOTE_REPLY
    }
}

#[derive(Copy, Clone, From, Debug)]
pub enum ConsensusMessage {
    Heartbeat(HeartbeatMessage),
    RequestVote(RequestVoteArgsMessage),
    VoteReply(VoteReplyMessage),
}

impl MessageDecoder for ConsensusMessage {
    type Message = Self;
    type Error = rmps::decode::Error;
    fn read_from(buf: &mut BytesMut, meta: &MessageHeaderDecoded) -> Result<Self::Message, Self::Error> {
        let mut reader = Cursor::new(buf.clone());
        let result = match meta.header.kind.as_str() {
            message_kind::HELLO => HeartbeatMessage::from_read(&mut reader),
            message_kind::REQUEST_VOTE => RequestVoteArgsMessage::from_read(&mut reader),
            message_kind::VOTE_REPLY => VoteReplyMessage::from_read(&mut reader),
            _ => Err(rmps::decode::Error::OutOfRange),
        };

        if let Ok(_) = result {
            buf.advance(reader.borrow().position() as usize);
        }

        result
    }
}
