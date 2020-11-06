use std::net::{SocketAddrV4, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::io::Cursor;
use std::str::FromStr;
use std::borrow::BorrowMut;

use bytes::{BytesMut, Buf};
use snafu::Error;
use net2::unix::UnixUdpBuilderExt;
use net2::UdpBuilder;
use log::{trace, debug, info, error};

use tokio::net::UdpSocket;
use tokio::sync::{broadcast, Mutex};
use tokio::net::udp::{SendHalf, RecvHalf};
use tokio::time::Duration;
use tokio::task::JoinHandle;

use crate::consensus::AtomicCancellation;
use crate::codec::{MessageDecoder, MessageHeader, MessageHeaderDecoded};

pub type NodeEndpointChannel<M> = broadcast::Sender<NodeEndpointNotification<M>>;

#[derive(Copy, Clone)]
pub struct NodeEndpointOptions {
    bind_addr: SocketAddrV4,
    multicast_addr: SocketAddrV4,
    heartbeat_send_interval: Duration,
    heartbeat_recv_timeout: Duration,
}

impl NodeEndpointOptions {
    pub fn multicast_addr(&self) -> SocketAddrV4 {
        self.multicast_addr
    }

    pub fn bind_addr(&self) -> SocketAddrV4 {
        self.bind_addr
    }

    pub fn heartbeat_send_interval(&self) -> Duration {
        self.heartbeat_send_interval
    }

    pub fn heartbeat_recv_timeout(&self) -> Duration {
        self.heartbeat_recv_timeout
    }
}

impl Default for NodeEndpointOptions {
    fn default() -> Self {
        let port = 6000u16;
        let listen_host = Ipv4Addr::from_str("0.0.0.0").unwrap();
        let multicast_address = Ipv4Addr::from_str("239.0.0.1").unwrap();

        NodeEndpointOptions {
            bind_addr: SocketAddrV4::new(listen_host, port),
            multicast_addr: SocketAddrV4::new(multicast_address, port),
            heartbeat_send_interval: Duration::from_millis(500),
            heartbeat_recv_timeout: Duration::from_millis(1250),
        }
    }
}

pub type NodeEndpointNotification<M> =
    (<M as MessageDecoder>::Message, SocketAddr);

pub type NodeEndpointNotificationSender<M> = broadcast::Sender<NodeEndpointNotification<M>>;

pub struct NodeEndpoint<M: MessageDecoder> {
    options: NodeEndpointOptions,
    send: Arc<Mutex<SendHalf>>,
    chan: Arc<NodeEndpointNotificationSender<M>>,
    recv_task: JoinHandle<()>,
    stop_signal: AtomicCancellation,
}

impl<M: MessageDecoder> NodeEndpoint<M> {
    fn clone_as_sender(&self) -> NodeEndpointReceiver<M> {
        NodeEndpointReceiver {
            chan: self.chan.clone(),
        }
    }
}

pub struct NodeEndpointReceiver<M: MessageDecoder> {
    chan: Arc<NodeEndpointNotificationSender<M>>,
}

impl<M> NodeEndpoint<M>
    where M: MessageDecoder,
          <M as MessageDecoder>::Message: 'static
{
    pub async fn start_any() -> Result<NodeEndpoint<M>, Box<dyn Error>> {
        Self::start(Default::default()).await
    }

    pub async fn start(options: NodeEndpointOptions) -> Result<NodeEndpoint<M>, Box<dyn Error>> {
        let udp_inner = UdpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind(options.bind_addr)?;

        // Enable listening on loopback for multicast messages -- this enables
        // multiple node executables to be running on the same machine.
        udp_inner.set_multicast_loop_v4(true)?;

        let socket = UdpSocket::from_std(udp_inner)?;
        socket.join_multicast_v4(*options.multicast_addr.ip(), *options.bind_addr.ip())?;
        let (recv, send) = socket.split();
        let (tx, _) = broadcast::channel(100);
        let tx = Arc::new(tx);
        let stop_signal = AtomicCancellation::new();

        let tx_clone = tx.clone();
        let stop_signal_clone = stop_signal.clone();
        let handle = tokio::spawn(async move {
            NodeEndpoint::<M>::handle_recv(recv, tx_clone, stop_signal_clone).await.unwrap();
        });

        Ok(NodeEndpoint {
            options,
            send: Arc::new(Mutex::new(send)),
            chan: tx.clone(),
            recv_task: handle,
            stop_signal,
        })
    }

    pub async fn subscribe(&self) -> broadcast::Receiver<NodeEndpointNotification<M>> {
        self.chan.subscribe()
    }

    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_signal.cancel();
        info!("consensus stop signal");
        self.recv_task.borrow_mut().await?;
        Ok(())
    }

    pub fn channel(&self) -> Arc<NodeEndpointNotificationSender<M>> {
        self.chan.clone()
    }

    pub fn socket_sender(&self) -> Arc<Mutex<SendHalf>> {
        self.send.clone()
    }

    async fn handle_recv(
        mut recv: RecvHalf,
        chan: Arc<NodeEndpointNotificationSender<M>>,
        stop_signal: AtomicCancellation,
    ) -> Result<(), Box<dyn Error>> {
        debug!("consensus endpoint receiver task started");

        let buf_size = 4 * 1024;
        let mut buf = BytesMut::with_capacity(buf_size);

        // IMPORTANT: Make sure that we resize the buffer so that when we convert
        // it to a slice (i.e. `revc_from(buf.as_mut())`), it will include the
        // zero'd capacity that is available to be written to. Otherwise, it will
        // think we are trying to use a buffer of zero length.
        buf.resize(buf_size, 0);
        debug!("buf len = {}", buf.len());

        // while (!stop_signal) { .. }
        while stop_signal.is_active() {
            buf.resize(buf_size, 0);
            let (num_read, sender) = recv.recv_from(buf.as_mut()).await.unwrap();
            buf.truncate(num_read);
            trace!("recv from {} ({} bytes)", sender, num_read);

            let mut header_cursor = Cursor::new(&buf);
            let header_result = rmps::from_read::<_, MessageHeader>(&mut header_cursor);
            let header = match header_result {
                Ok(header) => {
                    let header_len = header_cursor.position();
                    buf.advance(header_len as usize);
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
            trace!("Message received: {:?}", notify);
            chan.send(notify);
        }

        info!("consensus endpoint reader - shutdown!");
        Ok(())
    }
}
