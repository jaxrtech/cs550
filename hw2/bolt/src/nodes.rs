use tokio::net::TcpListener;

pub async fn try_listen_on(host: &str, mut port: u16) -> tokio::io::Result<TcpListener> {
    loop {
        let listen_address: String = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&listen_address).await;
        match &listener {
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                // Try again with any port
                port = 0;
                continue;
            },
            _ => {
                return listener;
            }
        }
    }
}