mod gemini;
mod protocol;

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{format_err, Context, Result};
use env_logger::Builder;
use log::LevelFilter;
use log::{error, info};
use sqlx::sqlite::SqlitePool;
use sqlx::{Done, Pool, Sqlite};
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::interval;

use gemini::check_feeds;
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

    async fn select_user(&mut self, username: String) -> Result<Vec<Response>> {
        let id = match sqlx::query!("SELECT id FROM users WHERE username = ?1", username)
            .fetch_one(self.pool)
            .await
        {
            Ok(user) => user
                .id
                .ok_or_else(|| format_err!("database entry for user \"{}\" has no ID", username))?,
            Err(_) => {
                let mut conn = self.pool.acquire().await?;

                sqlx::query!("INSERT INTO users (username) VALUES (?1)", username)
                    .execute(&mut conn)
                    .await?
                    .last_insert_rowid()
            }
        };

        self.user = ConnectedUser::User(username);

        Ok(vec![Response::AckUser { id }])
    }

    async fn add_feed(&self, name: String, url: String) -> Result<Vec<Response>> {
        let mut conn = self.pool.acquire().await?;

        let id = sqlx::query!("INSERT INTO feeds (name, url) VALUES (?1, ?2)", name, url)
            .execute(&mut conn)
            .await?
            .last_insert_rowid();

        Ok(vec![Response::AckAdd { id }])
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

    async fn remove_feed(&self, id: i64) -> Result<Vec<Response>> {
        let affected_rows = sqlx::query!("DELETE FROM feeds WHERE id = ?1", id)
            .execute(self.pool)
            .await?
            .rows_affected();

        if affected_rows > 0 {
            Ok(vec![Response::AckRemove])
        } else {
            Ok(vec![Response::ResourceNotFound(format!(
                "no feed with ID {} exists",
                id
            ))])
        }
    }

    async fn consume_command(&mut self, command: Command) -> Result<Vec<Response>> {
        info!("< {}", command);

        match command {
            Command::User { username } => self.select_user(username).await,
            Command::ListFeeds => self.list_feeds().await,
            Command::AddFeed { name, url } => self.add_feed(name, url).await,
            Command::RemoveFeed { id } => self.remove_feed(id).await,
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    address: SocketAddr,
    pool: &Pool<Sqlite>,
) -> Result<()> {
    let mut connection = Connection::new(address, pool);

    info!("Client connected from {}", connection.address);

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

struct Config {
    host_port: String,
    database_url: String,
    feed_fetch_interval: Duration,
}

async fn manage_feeds(pool: &Pool<Sqlite>, config: &Config) -> Result<()> {
    let mut timer = interval(config.feed_fetch_interval);
    timer.tick().await;

    loop {
        if let Err(e) = check_feeds(pool).await {
            error!("failed to check feeds: {}", e);
        }

        timer.tick().await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new().filter_level(LevelFilter::Info).init();

    let feed_fetch_interval_min =
        dotenv::var("FEED_FETCH_INTERVAL_MIN").unwrap_or_else(|_| "60".to_string());
    let feed_fetch_interval_min: u64 = feed_fetch_interval_min.parse().with_context(|| {
        format!(
            "invalid $FEED_FETCH_INTERVAL_MIN \"{}\"",
            feed_fetch_interval_min
        )
    })?;

    let config = Config {
        database_url: dotenv::var("DATABASE_URL").context("Missing env var $DATABASE_URL")?,
        host_port: dotenv::var("HOST_PORT").context("Missing env var $HOST_PORT")?,
        feed_fetch_interval: Duration::from_secs(feed_fetch_interval_min * 60),
    };

    let pool = SqlitePool::connect(&config.database_url).await?;

    let mut listener = TcpListener::bind(&config.host_port).await?;
    info!("Listening on: {}", config.host_port);

    {
        let pool = pool.clone();
        tokio::spawn(async move {
            manage_feeds(&pool, &config)
                .await
                .expect("feed manager failed");
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
