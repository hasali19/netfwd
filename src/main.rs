use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let listen_port = args.get(1).unwrap();
    let server = args.get(2).unwrap();
    let mut listener = TcpListener::bind(format!("0.0.0.0:{}", listen_port)).await?;

    println!("Listening at 0.0.0.0:{}", listen_port);

    loop {
        let (client, _) = listener.accept().await?;
        tokio::spawn(handle_connection(client, server.clone()));
    }
}

async fn handle_connection(mut client: TcpStream, server: String) {
    let mut server = TcpStream::connect(server).await.unwrap();
    let mut req_buf = [0; 4096];
    let mut res_buf = [0; 4096];
    loop {
        tokio::select! {
            Ok(len) = client.read(&mut req_buf) => {
                if len == 0 || server.write(&req_buf[..len]).await.is_err() {
                    break;
                }
            },
            Ok(len) = server.read(&mut res_buf) => {
                if len == 0 || client.write(&res_buf[..len]).await.is_err() {
                    break;
                }
            },
            else => {
                eprintln!("An error occurred while proxying the connection");
                break;
            }
        }
    }
}
