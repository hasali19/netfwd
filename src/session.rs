use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use log::{error, info, trace, warn};

pub struct Options {
    pub buf_size: usize,
    pub srv_addr: SocketAddr,
}

pub struct Session {
    cli: TcpStream,
    srv: TcpStream,
    cli_addr: SocketAddr,
    srv_addr: SocketAddr,
    req_buf: Vec<u8>,
    res_buf: Vec<u8>,
}

impl Session {
    pub async fn begin(cli: TcpStream, cli_addr: SocketAddr, options: Arc<Options>) -> Session {
        info!("connecting to server at '{}'", options.srv_addr);

        let srv = TcpStream::connect(options.srv_addr).await.unwrap();

        Session {
            cli,
            srv,
            cli_addr,
            srv_addr: options.srv_addr,
            req_buf: vec![0; options.buf_size],
            res_buf: vec![0; options.buf_size],
        }
    }

    pub async fn run(mut self) {
        info!("starting data transfer loop");

        loop {
            tokio::select! {
                Ok(len) = self.cli.read(&mut self.req_buf) => {
                    if len == 0 {
                        info!("client '{}' has disconnected", self.cli_addr);
                        break;
                    }

                    trace!(
                        "read {} bytes from client '{}', forwarding to server '{}'",
                        len, self.cli_addr, self.srv_addr
                    );

                    if self.srv.write(&self.req_buf[..len]).await.is_err() {
                        warn!("server '{}' disconnected unexpectedly", self.srv_addr);
                        break;
                    }
                },
                Ok(len) = self.srv.read(&mut self.res_buf) => {
                    if len == 0 {
                        info!("server '{}' has disconnected", self.srv_addr);
                        break;
                    }

                    trace!(
                        "read {} bytes from server '{}', forwarding to client '{}'",
                        len, self.srv_addr, self.cli_addr
                    );

                    if self.cli.write(&self.res_buf[..len]).await.is_err() {
                        warn!("client '{}' disconnected unexpectedly", self.cli_addr);
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
}
