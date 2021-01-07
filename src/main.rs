mod protocol;

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Result;
use env_logger::Builder;
use log::info;
use log::LevelFilter;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::interval;

use protocol::{Command, Response};

enum ConnectedUser {
    NoUser,
    User(String),
}

struct Connection {
    address: SocketAddr,
    user: ConnectedUser,
}

impl Connection {
    fn new(address: SocketAddr) -> Self {
        Self {
            address,
            user: ConnectedUser::NoUser,
        }
    }

    async fn consume_command(&mut self, command: Command) -> Result<Vec<Response>> {
        info!("< {}", command);

        match command {
            Command::User { username } => {
                self.user = ConnectedUser::User(username);
                Ok(vec![Response::AckUser])
            }
            Command::ListFeeds => Ok(vec![
                Response::StartFeedList,
                Response::Feed {
                    id: 42,
                    name: "test".to_string(),
                    url: "gemini://example.com".to_string(),
                },
                Response::EndList,
            ]),
            Command::AddFeed { name, url } => Ok(vec![Response::AckAdd { id: 69 }]),
            Command::RemoveFeed { id } => Ok(vec![Response::AckRemove]),
        }
    }
}

async fn handle_connection(stream: TcpStream, address: SocketAddr) -> Result<()> {
    info!("Client connected from {}", address);

    let mut connection = Connection::new(address);

    let (reader, mut writer) = tokio::io::split(stream);

    let server_reader = BufReader::new(reader);
    let mut lines = server_reader.lines();
    while let Some(line) = lines.next_line().await? {
        match line.parse() {
            Ok(command) => {
                let responses = connection.consume_command(command).await?;

                for response in responses.into_iter() {
                    writer
                        .write_all(format!("{}\r\n", response).as_bytes())
                        .await?;
                }
            }
            Err(e) => {
                let response: Response = e.into();
                writer
                    .write_all(format!("{}\r\n", response).as_bytes())
                    .await?;
            }
        }
    }

    info!("Client closed");

    Ok(())
}

async fn manage_feeds() -> Result<()> {
    // TODO(jsvana): config value
    let mut timer = interval(Duration::from_secs(30));
    timer.tick().await;

    loop {
        println!("asdf");

        timer.tick().await;
    }
}

// TODO(jsvana): config
#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter_level(LevelFilter::Info).init();

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let mut listener = TcpListener::bind(&addr).await?;
    info!("Listening on: {}", addr);

    tokio::spawn(manage_feeds());

    loop {
        let (stream, address) = listener.accept().await?;

        tokio::spawn(handle_connection(stream, address));
    }
}
