use std::convert::TryFrom;
use std::str::FromStr;

use anyhow::Result;
use chrono::NaiveDate;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;

use crate::gemini::fetch::Page;

lazy_static! {
    static ref ENTRY_REGEX: Regex =
        Regex::new(r"^=>\s+([^\s]+)\s+(\d{4}-\d{2}-\d{2})\s+(-\s+)?(.+)$").unwrap();
}

#[derive(Debug)]
pub struct Entry {
    published_at: NaiveDate,
    link: String,
    title: String,
}

#[derive(Debug, Error)]
pub enum ParseEntryError {
    #[error("malformed entry string")]
    MalformedEntry,
    #[error("missing year")]
    MissingYear,
    #[error("invalid year \"{0}\"")]
    InvalidYear(String),
    #[error("missing month")]
    MissingMonth,
    #[error("invalid month \"{0}\"")]
    InvalidMonth(String),
    #[error("missing day")]
    MissingDay,
    #[error("invalid day \"{0}\"")]
    InvalidDay(String),
}

impl FromStr for Entry {
    type Err = ParseEntryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let capture = ENTRY_REGEX
            .captures_iter(s)
            .next()
            .ok_or(ParseEntryError::MalformedEntry)?;

        let link = capture[1].to_string();
        let title = capture[4].to_string();

        let date_parts: Vec<&str> = capture[2].split('-').collect();

        let year = date_parts.get(0).ok_or(ParseEntryError::MissingYear)?;
        let year: i32 = year
            .parse()
            .map_err(|_| ParseEntryError::InvalidYear(year.to_string()))?;

        let month = date_parts.get(1).ok_or(ParseEntryError::MissingMonth)?;
        let month: u32 = month
            .parse()
            .map_err(|_| ParseEntryError::InvalidMonth(month.to_string()))?;

        let day = date_parts.get(2).ok_or(ParseEntryError::MissingDay)?;
        let day: u32 = day
            .parse()
            .map_err(|_| ParseEntryError::InvalidDay(day.to_string()))?;

        Ok(Entry {
            published_at: NaiveDate::from_ymd(year, month, day),
            link,
            title,
        })
    }
}

#[derive(Debug)]
pub struct Feed {
    base_url: String,
    title: String,
    subtitle: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Debug, Error)]
pub enum TryFromPageError {
    #[error("page is empty")]
    EmptyPage,
    #[error("header missing prefix (should be impossible)")]
    HeaderMissingPrefix,
    #[error("page is missing a title")]
    MissingTitle,
}

impl TryFrom<Page> for Feed {
    type Error = TryFromPageError;

    fn try_from(page: Page) -> Result<Self, Self::Error> {
        let body = page.body.ok_or(TryFromPageError::EmptyPage)?;

        let mut title: Option<String> = None;
        let mut title_line: Option<usize> = None;
        let mut subtitle: Option<String> = None;
        let mut entries = Vec::new();

        let lines = body.lines();

        for (i, line) in lines.enumerate() {
            if line.starts_with("# ") {
                if let None = title {
                    title = Some(
                        line.strip_prefix("# ")
                            .ok_or(TryFromPageError::HeaderMissingPrefix)?
                            .to_string(),
                    );
                    title_line = Some(i);
                }
            } else if line.starts_with("## ") {
                if let (None, Some(title_idx)) = (&subtitle, title_line) {
                    if title_idx == i - 1 {
                        subtitle = Some(
                            line.strip_prefix("## ")
                                .ok_or(TryFromPageError::HeaderMissingPrefix)?
                                .to_string(),
                        );
                    }
                }
            } else if line.starts_with("=> ") {
                if let Ok(entry) = line.parse::<Entry>() {
                    entries.push(entry);
                }
            }
        }

        Ok(Feed {
            base_url: page.url,
            title: title.ok_or(TryFromPageError::MissingTitle)?,
            subtitle,
            entries,
        })
    }
}
