use std::net::ToSocketAddrs;
use std::str::FromStr;

use anyhow::{format_err, Result};
use itertools::Itertools;
use native_tls::TlsConnector;
use sqlx::{Pool, Sqlite};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

const REDIRECT_CAP: usize = 5;

#[derive(Debug)]
enum Status {
    // 10
    Input,
    // 11
    SensitiveInput,
    // 20
    Success,
    // 30
    TemporaryRedirect,
    // 31
    PermanentRedirect,
    // 40
    TemporaryFailure,
    // 41
    ServerUnavailable,
    // 42
    CgiError,
    // 43
    ProxyError,
    // 44
    SlowDown,
    // 50
    PermanentFailure,
    // 51
    NotFound,
    // 52
    Gone,
    // 53
    ProxyRequestRefused,
    // 59
    BadRequest,
    // 60
    ClientCertificateRequired,
    // 61
    CertificateNotAuthorized,
    // 62
    CertificateNotValid,
}

#[derive(Debug, Error)]
enum ParseStatusError {
    #[error("invalid status \"{0}\"")]
    InvalidStatus(String),
}

impl FromStr for Status {
    type Err = ParseStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "10" => Ok(Status::Input),
            "11" => Ok(Status::SensitiveInput),
            "20" => Ok(Status::Success),
            "30" => Ok(Status::TemporaryRedirect),
            "31" => Ok(Status::PermanentRedirect),
            "40" => Ok(Status::TemporaryFailure),
            "41" => Ok(Status::ServerUnavailable),
            "42" => Ok(Status::CgiError),
            "43" => Ok(Status::ProxyError),
            "44" => Ok(Status::SlowDown),
            "50" => Ok(Status::PermanentFailure),
            "51" => Ok(Status::NotFound),
            "52" => Ok(Status::Gone),
            "53" => Ok(Status::ProxyRequestRefused),
            "59" => Ok(Status::BadRequest),
            "60" => Ok(Status::ClientCertificateRequired),
            "61" => Ok(Status::CertificateNotAuthorized),
            "62" => Ok(Status::CertificateNotValid),
            _ => Err(ParseStatusError::InvalidStatus(s.to_string())),
        }
    }
}

#[derive(Debug)]
struct Header {
    status: Status,
    meta: String,
}

#[derive(Debug, Error)]
enum ParseHeaderError {
    #[error("missing status")]
    MissingStatus,
    #[error("missing meta")]
    MissingMeta,
    #[error(transparent)]
    InvalidStatus(#[from] ParseStatusError),
}

impl FromStr for Header {
    type Err = ParseHeaderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim().split(' ').collect();

        let status: Status = parts
            .get(0)
            .ok_or(ParseHeaderError::MissingStatus)?
            .parse()?;
        let meta = parts.get(1).ok_or(ParseHeaderError::MissingMeta)?;

        Ok(Header {
            status,
            meta: meta.to_string(),
        })
    }
}

#[derive(Debug)]
struct Page {
    header: Header,
    body: Option<String>,
}

#[derive(Debug, Error)]
enum CheckFeedError {
    #[error("unsupported scheme for feed \"{0}\", only gemini is supported")]
    UnsupportedScheme(String),
    #[error("missing host in feed \"{0}\"")]
    MissingHost(String),
    #[error("failed to resolve feed \"{0}\"")]
    FailedToResolve(String),
    #[error("response is missing its header")]
    MissingHeader,
}

async fn fetch_page(full_url: String) -> Result<Page> {
    let feed_url = Url::parse(&full_url)?;

    if feed_url.scheme() != "gemini" {
        return Err(CheckFeedError::UnsupportedScheme(full_url.to_string()).into());
    }

    let host = feed_url
        .host_str()
        .ok_or_else(|| CheckFeedError::MissingHost(full_url.to_string()))?;
    let port = feed_url.port().unwrap_or(1965);

    let addr = format!("{}:{}", host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| CheckFeedError::FailedToResolve(full_url.to_string()))?;

    let socket = TcpStream::connect(&addr).await?;
    let cx = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let cx = tokio_native_tls::TlsConnector::from(cx);

    let mut socket = cx.connect(host, socket).await?;

    socket
        .write_all(format!("{}\r\n", full_url).as_bytes())
        .await?;

    let mut data = Vec::new();
    socket.read_to_end(&mut data).await?;

    let response = String::from_utf8(data)?;
    let mut response_lines = response.lines();

    let header: Header = response_lines
        .next()
        .ok_or(CheckFeedError::MissingHeader)?
        .parse()?;

    let body = response_lines.join("\n");

    Ok(Page {
        header,
        body: if body.is_empty() { None } else { Some(body) },
    })
}

async fn fetch_page_handle_redirects(full_url: String) -> Result<Page> {
    let mut url_to_fetch = full_url;

    let mut attempts = 0;
    while attempts < REDIRECT_CAP {
        let page = fetch_page(url_to_fetch).await?;

        if let Status::TemporaryRedirect | Status::PermanentRedirect = page.header.status {
            attempts += 1;
            url_to_fetch = page.header.meta;
        } else {
            return Ok(page);
        }
    }

    Err(format_err!(
        "reached maximum redirect cap of {}",
        REDIRECT_CAP
    ))
}

pub async fn check_feeds(_pool: &Pool<Sqlite>) -> Result<()> {
    let full_url = "gemini://gemini.circumlunar.space/".to_string();

    let contents = fetch_page_handle_redirects(full_url).await?;

    println!("{:?}", contents);

    Ok(())
}
