use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{lookup_host, TcpListener, TcpStream};

use env_logger::Env;
use log::{error, info, trace, warn};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let buffer_size = 81292;
    let args = std::env::args().collect::<Vec<_>>();
    let listen_port = args.get(1).unwrap();
    let server_addr = resolve_server(args.get(2).unwrap())
        .await
        .expect("failed to resolve server address");

    let mut listener = TcpListener::bind(format!("0.0.0.0:{}", listen_port)).await?;

    info!(
        "forwarding connections from '0.0.0.0:{}' to '{}'",
        listen_port, server_addr
    );

    loop {
        let (client, from) = listener.accept().await?;
        tokio::spawn(handle_connection(from, client, server_addr, buffer_size));
    }
}

async fn resolve_server(address: &str) -> Option<SocketAddr> {
    lookup_host(address)
        .await
        .ok()
        .and_then(|mut res| res.next())
}

async fn handle_connection(
    from: SocketAddr,
    mut client: TcpStream,
    server: SocketAddr,
    buffer_size: usize,
) {
    info!("accepted connection from '{}'", from);
    info!("connecting to server at '{}'", server);

    let mut server = TcpStream::connect(server).await.unwrap();

    let mut req_buf = vec![0; buffer_size];
    let mut res_buf = vec![0; buffer_size];

    let client_addr = client.peer_addr().unwrap();
    let server_addr = server.peer_addr().unwrap();

    info!("starting data transfer loop");

    loop {
        tokio::select! {
            Ok(len) = client.read(&mut req_buf) => {
                if len == 0 {
                    info!("client '{}' has disconnected", client_addr);
                    break;
                }

                trace!("read {} bytes from client '{}', forwarding to server '{}'", len, client_addr, server_addr);

                if server.write(&req_buf[..len]).await.is_err() {
                    warn!("server '{}' disconnected unexpectedly", server_addr);
                    break;
                }
            },
            Ok(len) = server.read(&mut res_buf) => {
                if len == 0 {
                    info!("server '{}' has disconnected", server_addr);
                    break;
                }

                trace!("read {} bytes from server '{}', forwarding to client '{}'", len, server_addr, client_addr);

                if client.write(&res_buf[..len]).await.is_err() {
                    warn!("client '{}' disconnected unexpectedly", client_addr);
                    break;
                }
            },
            else => {
                error!("an error occurred while proxying the connection");
                break;
            }
        }
    }

    info!("transfer has ended, closing any open connections");
}
