use std::fmt;
use std::str::FromStr;

use thiserror::Error;

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
//
// ##########
// # Errors #
// ##########
//
// 50 => BadCommand

pub enum Command {
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
pub enum ParseCommandError {
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

pub enum Response {
    AckUser,
    StartFeedList,
    Feed { id: i64, name: String, url: String },
    EndList,
    AckAdd { id: i64 },
    AckRemove,

    BadCommand(String),
}

impl From<ParseCommandError> for Response {
    fn from(e: ParseCommandError) -> Response {
        Response::BadCommand(e.to_string())
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Response::AckUser => write!(f, "20"),
            Response::StartFeedList => write!(f, "21"),
            Response::Feed { id, name, url } => write!(f, "22 {} {} {}", id, name, url),
            Response::EndList => write!(f, "23"),
            Response::AckAdd { id } => write!(f, "24 {}", id),
            Response::AckRemove => write!(f, "25"),

            Response::BadCommand(message) => write!(f, "50 {}", message),
        }
    }
}
