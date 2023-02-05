use thiserror::Error;

#[derive(Error, Debug)]
pub enum FilmwebErrors {
    #[error("title not found")]
    ZeroResults,
    #[error("couldn't fetch duration")]
    InvalidDuration,
    #[error("provided JWT is invalid / has invalidated, try again with a new one")]
    InvalidJwt,
    #[error("while parsing a year for title_id {}, string that caused that error: {}", .title_id, .failed_year)]
    InvalidYear { title_id: u32, failed_year: String },
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("while sending a request / building a client / parsing a response: {}", .source)]
    ReqwestError {
        #[from]
        source: reqwest::Error,
    },
    #[error("while inserting a cookie to a header: {}", .source)]
    InvalidHeaderValue {
        #[from]
        source: reqwest::header::InvalidHeaderValue,
    },
    #[error("while probably trying to convert an id string to int: {}", .source)]
    InvalidId {
        #[from]
        source: std::num::ParseIntError,
    },
}

#[derive(Error, Debug)]
pub enum FilmwebScrapeError {
    #[error("failed sending a request: {}", .source)]
    NetworkError {
        #[from]
        source: reqwest::Error,
    },
    #[error("Filmweb API has changed. Update or wait for an update")]
    FilmwebJsonApiChanged {
        #[from]
        source: serde_json::Error,
    },
    #[error("Filmed crate is outdated. Update or wait for an update")]
    Outdated,
}

#[derive(Error, Debug)]
pub enum IMDbScrapeError {
    #[error("no results in: {}", .search_url)]
    NoResults { search_url: String },
    #[error("failed sending a request: {}", .source)]
    NetworkError {
        #[from]
        source: reqwest::Error,
    },
    #[error("Filmed crate is outdated. Update or wait for an update")]
    IrrecoverableOutdated,
    #[error("Filmed crate is outdated. Update or wait for an update")]
    IrrecoverableParseYearError {
        title_url: String,
        source: ParseYearError,
    },
    #[error("Filmed crate is outdated. Update or wait for an update. Bad string: {}", .bad_string)]
    IrrecoverableParseDurationError { bad_string: String },
    #[error("Title {} contains no genres", .bad_title_url)]
    GenreParseError { bad_title_url: String },
}

#[derive(Error, Debug, PartialEq, Eq)]
#[error("Failed parsing year: {}", .year_str)]
pub struct ParseYearError {
    pub year_str: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseGenreError;
