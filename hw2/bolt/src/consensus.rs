mod messages;
mod endpoint;

use std::borrow::{BorrowMut};
use std::collections::{HashSet, HashMap};
use std::sync::{Arc};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use std::collections::hash_map::RandomState;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::error::Error;
use std::mem::ManuallyDrop;

use tokio::sync::{RwLock, broadcast, Mutex};
use tokio::time::{self, Duration, delay_for};
use tokio::net::{UdpSocket};
use tokio::net::udp::{RecvHalf, SendHalf};
use tokio::task::JoinHandle;

use rand::Rng;
use uuid::Uuid;
use log::{trace, debug, info, error};
use bytes::{BytesMut, Buf};
use evmap::{ReadHandle, WriteHandle, ShallowCopy};

use crate::codec::MessageEncoderToBuf;
use crate::consensus::ConsensusState::{Candidate, Follower};
use crate::consensus::messages::ConsensusMessage;
use crate::consensus::messages::ConsensusMessage::Heartbeat;
use crate::consensus::endpoint::{NodeEndpoint, NodeEndpointOptions, NodeEndpointNotificationSender};
use crate::util::unix_now;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ConsensusState {
    Follower,
    Candidate,
    Leader,
    Dead
}

fn get_election_timeout(rng: &mut impl Rng) -> Duration {
    Duration::from_millis(rng.gen_range(150, 300))
}

type NodeId = Uuid;
type TermIndex = u32;

struct LogEntry {
    term: TermIndex,
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

struct PeerStatus {
    last_addr: SocketAddrV4,
}

#[derive(Clone)]
struct AtomicCancellation {
    value: Arc<AtomicBool>,
}

impl AtomicCancellation {
    fn new() -> AtomicCancellation {
        AtomicCancellation {
            value: Arc::new(AtomicBool::new(false)),
        }
    }

    fn is_cancelled(&self) -> bool {
        (*self.value).load(Ordering::SeqCst)
    }

    fn is_active(&self) -> bool {
        !self.is_cancelled()
    }

    fn cancel(&self) {
        (*self.value).swap(true, Ordering::SeqCst);
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

type ConsensusPeerReader = evmap::ReadHandle<NodeId, Box<Duration>>;
type ConsensusPeerWriter = evmap::WriteHandle<NodeId, Box<Duration>>;

pub struct Consensus {
    /// The ID of this node
    id: NodeId,
    ctx: tokio::sync::RwLock<ConsensusInternal>,
    endpoint: NodeEndpoint<ConsensusMessage>,
    tracking_handle: JoinHandle<()>,
    heartbeat_handle: JoinHandle<()>,
    peers_r: ConsensusPeerReader,
}


impl Consensus {
    pub async fn start(options: NodeEndpointOptions) -> Result<Consensus, Box<dyn Error>> {
        debug!("starting consensus...");

        let node_id = Uuid::new_v4();
        info!("node id = {}", node_id);

        let (peer_r, mut peer_w) = evmap::new();
        let endpoint = NodeEndpoint::<ConsensusMessage>::start(options).await?;

        let tracking_broadcast = endpoint.channel();
        let tracking_handle = tokio::spawn(
            Self::track_peer_heartbeats(peer_w, tracking_broadcast, node_id));

        let heartbeat_socket = endpoint.socket_sender();
        let heartbeat_handle = tokio::spawn(
            Self::send_heartbeats(heartbeat_socket, node_id, options));

        Ok(Consensus {
            id: node_id,
            ctx: RwLock::new(ConsensusInternal::new()),
            endpoint,
            tracking_handle,
            heartbeat_handle,
            peers_r: peer_r
        })
    }

    async fn track_peer_heartbeats(
        mut peers_w: ConsensusPeerWriter,
        chan: Arc<NodeEndpointNotificationSender<ConsensusMessage>>,
        self_id: NodeId,
    ) {
        let mut subscription = chan.subscribe();
        loop {
            let notification = subscription.recv().await;
            match notification {
                Ok((ConsensusMessage::Heartbeat(msg), _addr)) => {
                    let node_id = msg.node_id;
                    let now = unix_now();

                    // Ignore heartbeats from ourself
                    if node_id == self_id {
                        continue
                    }

                    peers_w.clear(node_id);
                    peers_w.insert(node_id, Box::new(now));
                    peers_w.refresh();
                    debug!("peer {} - received heartbeat at {:#?}", node_id, now);
                }
                Ok(_) => {}
                Err(e) => {
                    error!("failed to receive heartbeat: {}", e);
                }
            }
        }
    }

    async fn send_heartbeats(mut socket: Arc<Mutex<SendHalf>>, node_id: NodeId, options: NodeEndpointOptions) {
        let mut buf = Vec::with_capacity(4 * 1024);
        let addr = SocketAddr::V4(options.multicast_addr());
        loop {
            let mut cursor = Cursor::new(&mut buf);
            let msg = messages::HeartbeatMessage { node_id };
            msg.write_to_blocking(&mut cursor);
            cursor.set_position(0);

            {
                let mut socket = socket.lock().await;
                socket.send_to(cursor.bytes(), &addr).await;
            }

            drop(cursor);
            buf.clear();
            trace!("send heartbeat message");

            delay_for(options.heartbeat_send_interval()).await;
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

