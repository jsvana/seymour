mod protocol;

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{format_err, Result};
use env_logger::Builder;
use log::LevelFilter;
use log::{error, info};
use sqlx::sqlite::SqlitePool;
use sqlx::{Pool, Sqlite};
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::interval;

use protocol::{Command, Response};

enum ConnectedUser {
    NoUser,
    User(String),
}

struct Connection<'a> {
    address: SocketAddr,
    user: ConnectedUser,
    pool: &'a Pool<Sqlite>,
}

impl<'a> Connection<'a> {
    fn new(address: SocketAddr, pool: &'a Pool<Sqlite>) -> Self {
        Self {
            address,
            user: ConnectedUser::NoUser,
            pool,
        }
    }

    async fn list_feeds(&self) -> Result<Vec<Response>> {
        let feeds = sqlx::query!(
            r#"
            SELECT id, name, url
            FROM feeds
            "#
        )
        .fetch_all(self.pool)
        .await?;

        let mut responses = vec![Response::StartFeedList];

        for feed in feeds {
            responses.push(Response::Feed {
                id: feed
                    .id
                    .ok_or_else(|| format_err!("feed with name \"{}\" has no ID", feed.name))?,
                name: feed.name,
                url: feed.url,
            });
        }

        responses.push(Response::EndList);

        Ok(responses)
    }

    async fn consume_command(&mut self, command: Command) -> Result<Vec<Response>> {
        info!("< {}", command);

        match command {
            Command::User { username } => {
                self.user = ConnectedUser::User(username);
                Ok(vec![Response::AckUser])
            }
            Command::ListFeeds => self.list_feeds().await,
            Command::AddFeed {
                name: _name,
                url: _url,
            } => Ok(vec![Response::AckAdd { id: 69 }]),
            Command::RemoveFeed { id: _id } => Ok(vec![Response::AckRemove]),
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    address: SocketAddr,
    pool: &Pool<Sqlite>,
) -> Result<()> {
    info!("Client connected from {}", address);

    let mut connection = Connection::new(address, pool);

    let (reader, mut writer) = tokio::io::split(stream);

    let server_reader = BufReader::new(reader);
    let mut lines = server_reader.lines();
    while let Some(line) = lines.next_line().await? {
        match line.parse() {
            Ok(command) => match connection.consume_command(command).await {
                Ok(responses) => {
                    for response in responses.into_iter() {
                        writer
                            .write_all(format!("{}\r\n", response).as_bytes())
                            .await?;
                    }
                }
                Err(e) => {
                    writer
                        .write_all(
                            format!("{}\r\n", Response::InternalError(e.to_string())).as_bytes(),
                        )
                        .await?;
                }
            },
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

async fn manage_feeds(_pool: &Pool<Sqlite>) -> Result<()> {
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

    let pool = SqlitePool::connect("sqlite://seymour.db").await?;

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let mut listener = TcpListener::bind(&addr).await?;
    info!("Listening on: {}", addr);

    {
        let pool = pool.clone();
        tokio::spawn(async move {
            manage_feeds(&pool).await.expect("feed manager failed");
        });
    }

    loop {
        let (stream, address) = listener.accept().await?;

        let pool = pool.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, address, &pool).await {
                error!("client handler failed: {}", e);
            }
        });
    }
}
