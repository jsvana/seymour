use std::fmt;
use std::str::FromStr;

use thiserror::Error;

// ############
// # Protocol #
// ############
//
// [connect]
// > USER <username>
// < 20 <user_id>
// > LISTFEEDS
// < 21
// < 22 <feed_id> <feed_url> :<feed_name>
// < 25
// > LISTUNREAD
// < 23
// < 24 <entry_id> <feed_id> <feed_url> <entry_title> :<entry_link>
// < 25
// > MARKREAD <entry_id>
// < 28

/// Commands sent to seymour server
pub enum Command {
    /// Select the user user
    User { username: String },

    /// List the current user's subscriptions
    ///
    /// Requires a client to issue a User
    /// command prior.
    ListSubscriptions,

    /// Subscribe the current user to a new feed
    ///
    /// Requires a client to issue a User
    /// command prior.
    Subscribe { url: String },

    /// Unsubscribe the current user from a feed
    ///
    /// Requires a client to issue a User
    /// command prior.
    Unsubscribe { id: i64 },

    /// List the current user's unread feed entries
    ///
    /// Requires a client to issue a User
    /// command prior.
    ListUnread,

    /// Mark a feed entry as read by the current user
    ///
    /// Requires a client to issue a User
    /// command prior.
    MarkRead { id: i64 },
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Command::User { username } => write!(f, "USER {}", username),
            Command::ListSubscriptions => write!(f, "LISTSUBSCRIPTIONS"),
            Command::Subscribe { url } => write!(f, "SUBSCRIBE {}", url),
            Command::Unsubscribe { id } => write!(f, "UNSUBSCRIBE {}", id),
            Command::ListUnread => write!(f, "LISTUNREAD"),
            Command::MarkRead { id } => write!(f, "MARKREAD {}", id),
        }
    }
}

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
            "LISTSUBSCRIPTIONS" => {
                check_arguments(&parts, 0)?;

                Ok(Command::ListSubscriptions)
            }
            "SUBSCRIBE" => {
                check_arguments(&parts, 1)?;

                let url = parts
                    .get(1)
                    .ok_or_else(|| ParseCommandError::MissingArgument("url".to_string()))?;

                Ok(Command::Subscribe {
                    url: url.to_string(),
                })
            }
            "UNSUBSCRIBE" => {
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

                Ok(Command::Unsubscribe { id })
            }
            "LISTUNREAD" => {
                check_arguments(&parts, 0)?;

                Ok(Command::ListUnread)
            }
            "MARKREAD" => {
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

                Ok(Command::MarkRead { id })
            }
            _ => Err(ParseCommandError::UnknownCommand(command.to_string())),
        }
    }
}

/// Responses sent from seymour server
pub enum Response {
    /// Acknowledgement for selecting current user
    AckUser { id: i64 },

    /// Beginning of a list of subscriptions
    ///
    /// Must be followed by zero or more Subscription lines and
    /// one EndList.
    StartSubscriptionList,

    /// A single subscription entry
    ///
    /// Must be preceeded by one StartSubscriptionList and
    /// followed by one EndList.
    Subscription { id: i64, url: String },

    /// Beginning of a list of feed entries
    ///
    /// Must be followed by zero or more Entry lines and
    /// one EndList.
    StartEntryList,

    /// A single feed entry
    ///
    /// Must be preceeded by one StartEntryList and
    /// followed by one EndList.
    Entry {
        id: i64,
        feed_id: i64,
        feed_url: String,
        title: String,
        url: String,
    },

    /// Ends a list sent by the server
    ///
    /// Must be preceeded by at least either a StartSubscriptionList
    /// or a StartEntryList.
    EndList,

    /// Acknowledgement for subscribing the current user
    /// to a new feed
    AckSubscribe,

    /// Acknowledgement for unsubscribing the current user
    /// from a feed
    AckUnsubscribe,

    /// Acknowledgement for marking a feed entry as read
    /// by the current user
    AckMarkRead,

    /// Error stating that the specified resource was
    /// not found
    ResourceNotFound(String),

    /// Error stating that the command sent was not valid
    BadCommand(String),

    /// Error stating that the command sent requires a
    /// selected user, but no user has been selected
    NeedUser(String),

    /// Error stating that the seymour server hit an
    /// internal problem while attempting to serve
    /// the request
    InternalError(String),
}

impl From<ParseCommandError> for Response {
    fn from(e: ParseCommandError) -> Response {
        Response::BadCommand(e.to_string())
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Response::AckUser { id } => write!(f, "20 {}", id),
            Response::StartSubscriptionList => write!(f, "21"),
            Response::Subscription { id, url } => write!(f, "22 {} {}", id, url),
            Response::StartEntryList => write!(f, "23"),
            Response::Entry {
                id,
                feed_id,
                feed_url,
                title,
                url,
            } => write!(f, "24 {} {} {} {} :{}", id, feed_id, feed_url, url, title),
            Response::EndList => write!(f, "25"),
            Response::AckSubscribe => write!(f, "26"),
            Response::AckUnsubscribe => write!(f, "27"),
            Response::AckMarkRead => write!(f, "28"),

            Response::ResourceNotFound(message) => write!(f, "40 {}", message),
            Response::BadCommand(message) => write!(f, "41 {}", message),
            Response::NeedUser(message) => write!(f, "42 {}", message),

            Response::InternalError(message) => write!(f, "51 {}", message),
        }
    }
}
