use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use std::collections::hash_map::RandomState;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::error::Error;

use tokio::sync::{RwLock, broadcast};
use tokio::time::{self, Duration};
use tokio::net::{UdpSocket};
use tokio::net::udp::{RecvHalf, SendHalf};
use tokio::task::JoinHandle;

use rand::Rng;
use uuid::Uuid;
use log::{debug};
use bytes::BytesMut;

use crate::codec::{MessageHeader, MessageHeaderDecoded, MessageDecoder};
use crate::consensus::ConsensusState::{Candidate, Follower};
use std::borrow::{BorrowMut};

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ConsensusState {
    Follower,
    Candidate,
    Leader,
    Dead
}

fn unix_now() -> Duration {
    let now = SystemTime::now();
    let unix_timestamp = now
        .duration_since(UNIX_EPOCH)
        .expect("System time went backwards");

    unix_timestamp
}

fn get_election_timeout(rng: &mut impl Rng) -> Duration {
    Duration::from_millis(rng.gen_range(150, 300))
}

type NodeId = Uuid;
type TermIndex = u32;

struct LogEntry {
    term: TermIndex,
}

mod messages {
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

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    pub struct HelloMessage {
        pub node_id: NodeId,
    }

    impl MessageKindTagged for HelloMessage {
        fn kind(&self) -> &'static str {
            message_kind::HELLO
        }
    }

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
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

    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    pub struct VoteReplyMessage {
        pub term: TermIndex,
        pub vote_graded: bool,
    }

    impl MessageKindTagged for VoteReplyMessage {
        fn kind(&self) -> &'static str {
            message_kind::VOTE_REPLY
        }
    }

    #[derive(From, Debug)]
    pub enum MessageBody {
        Hello(HelloMessage),
        RequestVote(RequestVoteArgsMessage),
        VoteReply(VoteReplyMessage),
    }

    impl MessageDecoder for MessageBody {
        type Message = Self;
        type Error = rmps::decode::Error;
        fn read_from(buf: &mut BytesMut, meta: &MessageHeaderDecoded) -> Result<Self::Message, Self::Error> {
            let mut reader = Cursor::new(buf.clone());
            let result = match meta.header.kind.as_str() {
                message_kind::HELLO => HelloMessage::from_read(&mut reader),
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
}

struct ConsensusInternal {
    /// The IDs of the peers in the cluster
    peer_ids: HashSet<NodeId, RandomState>,

    // Persistent state for all nodes
    current_term: TermIndex,
    voted_for: Option<NodeId>,
    log: Vec<TermIndex>,

    // Volatile state for all nodes
    commit_index: TermIndex,
    last_applied: TermIndex,
    state: ConsensusState,
    election_reset_event: Instant,

    // Volatile state for leaders
    next_index: HashMap<NodeId, TermIndex>,
    match_index: HashMap<NodeId, TermIndex>,
}

impl ConsensusInternal {
    fn new() -> ConsensusInternal {
        ConsensusInternal {
            peer_ids: HashSet::new(),

            current_term: 0,
            voted_for: None,
            log: Vec::new(),

            commit_index: 0,
            last_applied: 0,
            state: ConsensusState::Follower,
            election_reset_event: Instant::now(),

            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }
}

pub struct Consensus {
    /// The ID of this node
    id: NodeId,
    ctx: RwLock<ConsensusInternal>,
}

type NodeEndpointChannel<M> = broadcast::Sender<NodeEndpointNotification<M>>;

struct NodeEndpoint<M: MessageDecoder> {
    send: SendHalf,
    chan: Arc<broadcast::Sender<NodeEndpointNotification<M>>>,
    recv_task: JoinHandle<()>,
    stop_signal: Arc<AtomicBool>,
}

#[derive(Copy, Clone)]
struct NodeEndpointOptions {
    bind_address: SocketAddrV4,
    multicast_address: Ipv4Addr,
}

impl Default for NodeEndpointOptions {
    fn default() -> Self {
        let listen_host = Ipv4Addr::from_str("0.0.0.0").unwrap();
        let port = 24000u16;
        let multicast_address = Ipv4Addr::from_str("239.0.0.1").unwrap();
        NodeEndpointOptions {
            bind_address: SocketAddrV4::new(listen_host, port),
            multicast_address
        }
    }
}

struct PeerStatus {
    last_addr: SocketAddrV4,
}

type NodeEndpointNotification<M> =
    (<M as MessageDecoder>::Message, SocketAddr);

impl<M> NodeEndpoint<M>
    where M: MessageDecoder,
          <M as MessageDecoder>::Message: 'static
{
    async fn start_any() -> Result<NodeEndpoint<M>, Box<dyn Error>> {
        Self::start(Default::default()).await
    }

    async fn start(options: NodeEndpointOptions) -> Result<NodeEndpoint<M>, Box<dyn Error>> {
        let socket = UdpSocket::bind(options.bind_address).await?;
        socket.join_multicast_v4(options.multicast_address, *options.bind_address.ip())?;
        let (recv, send) = socket.split();
        let (tx, _) = broadcast::channel(100);
        let tx = Arc::new(tx);
        let stop_signal = Arc::new(AtomicBool::new(false));

        let tx_clone = tx.clone();
        let stop_signal_clone = stop_signal.clone();
        let handle = tokio::spawn(async move {
            NodeEndpoint::<M>::handle_recv(recv, tx_clone, stop_signal_clone).await.unwrap();
        });

        Ok(NodeEndpoint {
            send,
            chan: tx.clone(),
            recv_task: handle,
            stop_signal,
        })
    }

    async fn subscribe(&self) -> broadcast::Receiver<NodeEndpointNotification<M>> {
        self.chan.subscribe()
    }

    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_signal.swap(true, Ordering::Relaxed);
        self.recv_task.borrow_mut().await?;
        Ok(())
    }

    async fn handle_recv(
        mut recv: RecvHalf,
        chan: Arc<broadcast::Sender<NodeEndpointNotification<M>>>,
        stop_signal: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn Error>> {
        let mut buf = BytesMut::with_capacity(4 * 1024);

        // while (!stop_signal) { .. }
        while (*stop_signal).fetch_nand(true, Ordering::Relaxed) {
            buf.clear();
            let (_, sender) = recv.recv_from(&mut buf).await?;

            let mut header_cursor = Cursor::new(&buf);
            let header_result = rmps::from_read::<_, MessageHeader>(&mut header_cursor);
            let header = match header_result {
                Ok(header) => {
                    let header_len = header_cursor.position();
                    buf.split_to(header_len as usize);
                    MessageHeaderDecoded { header, header_len: header_len as u32 }
                }
                Err(e) => {
                    debug!("Bad message header format: {}", e);
                    continue;
                }
            };

            let body_result = M::read_from(&mut buf, &header);
            let body = match body_result {
                Ok(body) => body,
                Err(e) => {
                    debug!("Bad message body format: {}", e);
                    continue;
                }
            };

            let notify = (body, sender);
            chan.send(notify);
        }

        Ok(())
    }
}

// async fn handle_message(body: messages::MessageBody) {
//
//     match body {
//         MessageBody::RequestVote(_) => {},
//         MessageBody::Hello(hello) => {
//             let mut lock = cache.write().await;
//             lock.insert(hello.node_id, sender, heartbeat_lifespan);
//         }
//         MessageBody::VoteReply(reply) => {}
//     }
// }

impl Consensus {
    fn new() -> Consensus {
        Consensus {
            id: Uuid::new_v4(),
            ctx: RwLock::new(ConsensusInternal::new())
        }
    }

    async fn run_election_timer(&mut self) {
        let mut rng = rand::thread_rng();
        let timeout = get_election_timeout(&mut rng);

        let lock = self.ctx.read().await;
        let starting_term = lock.current_term;
        drop(lock);

        debug!("Election timer started ({:#?}), term = {}", timeout, starting_term);

        // Loop until one of the following happens:
        //  - the election is not longer needed
        //  - the election timer expires and this node becomes a candidate
        //
        // For a follower node, this will keep running in the background.
        //
        let mut interval = time::interval(Duration::from_millis(10));
        loop {
            interval.tick().await;

            let lock = self.ctx.read().await;
            let state = lock.state;
            let current_term = lock.current_term;
            let election_reset_event = lock.election_reset_event;
            drop(lock);

            if state != Candidate && state != Follower {
                debug!("Election state = {:?}. Bailing.", state);
                return
            }

            if starting_term != current_term {
                debug!("Election term changed from {} to {}. Bailing.", starting_term, current_term);
                return
            }

            // Otherwise, start an election if we haven't heard from a leader
            // or haven't voted for something during the timeout.
            //
            let elapsed = Instant::now() - election_reset_event;
            if elapsed > timeout {
                self.start_election().await;
            }
        }
    }

    async fn start_election(&self) {
        let mut ctx = self.ctx.write().await;
        ctx.state = Candidate;
        ctx.current_term += 1;
        let current_term_snapshot = ctx.current_term;
        ctx.election_reset_event = Instant::now();
        ctx.voted_for = Some(self.id);
        debug!("Transitioned to Candidate (current term = {}) | log = {:#?}", current_term_snapshot, ctx.log);
        drop(ctx);

        let mut votes_received = 1u32;

    }
}

