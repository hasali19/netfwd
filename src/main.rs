mod session;

use std::{net::SocketAddr, sync::Arc};

use env_logger::Env;
use log::info;
use tokio::net::{lookup_host, TcpListener};

use crate::session::{Options, Session};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let args = std::env::args().collect::<Vec<_>>();
    let listen_port = args.get(1).unwrap();
    let srv_addr = resolve_server(args.get(2).unwrap())
        .await
        .expect("failed to resolve server address");

    let mut listener = TcpListener::bind(format!("0.0.0.0:{}", listen_port)).await?;
    let options = Arc::new(Options {
        buf_size: 8192,
        srv_addr,
    });

    info!(
        "forwarding connections from '0.0.0.0:{}' to '{}'",
        listen_port, srv_addr
    );

    loop {
        let (cli, cli_addr) = listener.accept().await?;
        let options = Arc::clone(&options);

        info!("accepted connection from '{}'", cli_addr);

        tokio::spawn(async move {
            Session::begin(cli, cli_addr, options).await.run().await;
        });
    }
}

async fn resolve_server(address: &str) -> Option<SocketAddr> {
    lookup_host(address)
        .await
        .ok()
        .and_then(|mut res| res.next())
}
