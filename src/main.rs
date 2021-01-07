use std::env;
use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use env_logger::Builder;
use log::info;
use log::LevelFilter;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader, ReadHalf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::interval;

// ############
// # Protocol #
// ############
//
// [connect]
// > USER <username>
// < ACKUSER
// > LISTFEEDS
// < FEED id feedname url
// < ENDLIST
// > ADDFEED <feedname> <url>
// < ACKADD <id>
// > REMOVEFEED <id>
//
// [connect]
// > LISTFEEDS
// < NEEDUSER

enum Command {
    User { username: String },
    ListFeeds,
    AddFeed { name: String, url: String },
    RemoveFeed { id: i64 },
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::User { username } => write!(f, "USER {}", username),
            Command::ListFeeds => write!(f, "LISTFEEDS"),
            Command::AddFeed { name, url } => write!(f, "ADDFEED {} {}", name, url),
            Command::RemoveFeed { id } => write!(f, "REMOVEFEED {}", id),
        }
    }
}

// TODO(jsvana): impl From<ParseCommandError> for Response
#[derive(Debug, Error)]
enum ParseCommandError {
    #[error("empty command")]
    EmptyCommand,
    #[error("unknown command \"{0}\"")]
    UnknownCommand(String),
    #[error("missing argument \"{0}\"")]
    MissingArgument(String),
    #[error("too many arguments (expected {expected}, got {actual})")]
    TooManyArguments { expected: usize, actual: usize },
    #[error("invalid integer value \"{value}\" for argument \"{argument}\"")]
    InvalidIntegerArgument { argument: String, value: String },
}

fn check_arguments(parts: &Vec<&str>, expected: usize) -> Result<(), ParseCommandError> {
    if parts.len() > expected + 1 {
        return Err(ParseCommandError::TooManyArguments {
            expected,
            actual: parts.len() - 1,
        });
    }

    Ok(())
}

impl FromStr for Command {
    type Err = ParseCommandError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = value.split(' ').collect();

        let command = parts.get(0).ok_or(ParseCommandError::EmptyCommand)?;

        match *command {
            "USER" => {
                check_arguments(&parts, 1)?;

                let username = parts
                    .get(1)
                    .ok_or_else(|| ParseCommandError::MissingArgument("username".to_string()))?;

                Ok(Command::User {
                    username: username.to_string(),
                })
            }
            "LISTFEEDS" => {
                check_arguments(&parts, 0)?;

                Ok(Command::ListFeeds)
            }
            "ADDFEED" => {
                check_arguments(&parts, 2)?;

                let name = parts
                    .get(1)
                    .ok_or_else(|| ParseCommandError::MissingArgument("name".to_string()))?;
                let url = parts
                    .get(2)
                    .ok_or_else(|| ParseCommandError::MissingArgument("url".to_string()))?;

                Ok(Command::AddFeed {
                    name: name.to_string(),
                    url: url.to_string(),
                })
            }
            "REMOVEFEED" => {
                check_arguments(&parts, 1)?;

                let possible_id = parts
                    .get(1)
                    .ok_or_else(|| ParseCommandError::MissingArgument("id".to_string()))?;

                let id: i64 =
                    possible_id
                        .parse()
                        .map_err(|_| ParseCommandError::InvalidIntegerArgument {
                            argument: "id".to_string(),
                            value: possible_id.to_string(),
                        })?;

                Ok(Command::RemoveFeed { id })
            }
            _ => Err(ParseCommandError::UnknownCommand(command.to_string())),
        }
    }
}

enum Response {
    AckUser,
    Feed { id: i64, name: String, url: String },
    EndList,
    AckAdd { id: i64 },
    AckRm,

    BadCommand(String),
}

// ##########
// # Errors #
// ##########
//
// 50 => BadCommand

impl From<ParseCommandError> for Response {
    fn from(e: ParseCommandError) -> Response {
        Response::BadCommand(e.to_string())
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Response::AckUser => write!(f, "ACKUSER"),
            Response::Feed { id, name, url } => write!(f, "LISTFEEDS {} {} {}", id, name, url),
            Response::EndList => write!(f, "ENDLIST"),
            Response::AckAdd { id } => write!(f, "ACKADD {}", id),
            Response::AckRm => write!(f, "ACKRM"),

            Response::BadCommand(message) => write!(f, "50 {}", message),
        }
    }
}

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

    async fn consume_command(&mut self, command: Command) -> Result<()> {
        info!("< {}", command);

        Ok(())
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
            Ok(command) => connection.consume_command(command).await?,
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
