mod session;

use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;

use clap::{App, Arg};
use env_logger::Env;
use futures::future::join_all;
use log::{error, info};
use tokio::net::{lookup_host, TcpListener};

use crate::session::{Options, Session};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let matches = App::new("netfwd")
        .arg(
            Arg::with_name("forward")
                .short("f")
                .required(true)
                .multiple(true)
                .number_of_values(1)
                .empty_values(false),
        )
        .arg(
            Arg::with_name("buffer-size")
                .long("buffer-size")
                .help("Size in bytes of the request and response buffers for each connection")
                .number_of_values(1)
                .empty_values(false)
                .default_value("8192"),
        )
        .get_matches();

    let forwards = matches.values_of("forward").unwrap();
    let buffer_size = matches.value_of("buffer-size").unwrap().parse().unwrap();

    let mut tasks = vec![];

    for (src, dst) in forwards.map(parse_forward_arg) {
        let src = parse_src_addr(src);
        let dst = resolve_server_addr(dst).await;
        let task = tokio::spawn(start_proxy(src, dst, buffer_size));
        tasks.push(task);
    }

    join_all(tasks).await;

    Ok(())
}

fn parse_forward_arg(arg: &str) -> (&str, &str) {
    let mut parts = arg.split('=');
    let res = (|| Some((parts.next()?, parts.next()?)))();
    match res {
        Some(v) => v,
        None => {
            eprintln!("error: Values for '--forward' must be of the form:\n");
            eprintln!("    [bind_address]:port=server_address:port\n");
            exit(1);
        }
    }
}

fn parse_src_addr(addr: &str) -> SocketAddr {
    // Try parsing the address as just a port number...
    match addr.parse::<u16>() {
        Ok(port) => format!("0.0.0.0:{}", port).parse().unwrap(),
        // ...if that fails, parse it as a full address
        Err(_) => match addr.parse() {
            Ok(addr) => addr,
            Err(_) => {
                eprintln!("error: Failed to parse address '{}'", addr);
                exit(1);
            }
        },
    }
}

async fn resolve_server_addr(addr: &str) -> SocketAddr {
    match lookup_host(addr).await.ok().and_then(|mut res| res.next()) {
        Some(addr) => addr,
        None => {
            eprintln!("error: Failed to resolve address '{}'", addr);
            exit(1);
        }
    }
}

async fn start_proxy(from: SocketAddr, to: SocketAddr, buffer_size: usize) {
    let mut listener = match TcpListener::bind(from).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("failed to bind socket at '{}': {}", from, e);
            exit(1);
        }
    };

    let options = Arc::new(Options {
        buf_size: buffer_size,
        srv_addr: to,
    });

    info!("forwarding connections from '{}' to '{}'", from, to);

    loop {
        let (cli, cli_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(_) => {
                error!("failed to accept connection on '{}'", from);
                return;
            }
        };

        info!("accepted connection from '{}' on '{}'", cli_addr, from);

        tokio::spawn({
            let options = Arc::clone(&options);
            async move {
                if let Some(session) = Session::begin(cli, cli_addr, options).await {
                    session.run().await;
                }
            }
        });
    }
}
