pub mod error;
pub mod imdb;
mod utils;

use core::fmt;
pub use error::FwErrors;
use priority_queue::PriorityQueue;
use reqwest::blocking::Client;
use reqwest::header;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Deref};
use utils::ScrapedFwTitleData;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:106.0) Gecko/20100101 Firefox/108.0";

pub trait Title {
    fn url(&self) -> &String;

    fn id(&self) -> u32;

    fn title_pl(&self) -> &String;

    fn title_type(&self) -> &FwTitleType;

    fn duration(&self) -> Option<u16>;

    fn year(&self) -> &Year;

    fn alter_titles(&mut self) -> Option<&mut PriorityQueue<AlternateTitle, u8>>;

    fn set_imdb_data_with_lookup(&mut self, client: &Client) -> Result<(), FwErrors>;

    fn imdb_data(&self) -> Option<&imdb::Title>;

    fn imdb_data_owned(&mut self) -> Option<imdb::Title>;

    fn imdb_lookup(&mut self, client: &Client) -> Result<imdb::Title, FwErrors> {
        let year = match &mut self.year() {
            Year::OneYear(year) | Year::Range(year, _) => *year,
        };

        while let Some((ref alternate_title, score)) = self.alter_titles().as_mut().unwrap().pop() {
            if score == 0 {
                break;
            }

            if let Ok(imdb_title) =
                imdb::advanced_search(&alternate_title.title, year, year, client)
            {
                return Ok(imdb_title);
            }

            if let Ok(imdb_title) = imdb::search(&alternate_title.title, year, client) {
                return Ok(imdb_title);
            }
        }
        Err(FwErrors::ZeroResults)
    }
}

pub struct FwGuest(Client);

impl Deref for FwGuest {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn scrape_from_document(
    votebox: ElementRef,
    client: &Client,
) -> Result<ScrapedFwTitleData, FwErrors> {
    let id = votebox
        .select(&Selector::parse(".previewFilm").unwrap())
        .next()
        .unwrap()
        .value()
        .attr("data-film-id")
        .unwrap()
        .trim()
        .parse::<u32>()?;
    let year = {
        let year = votebox
            .select(&Selector::parse(".preview__year").unwrap())
            .next()
            .unwrap()
            .inner_html();
        if year.contains('-') {
            let years = year.trim().split('-').collect::<Vec<&str>>();
            let year_start = years[0]
                .trim()
                .parse::<u16>()
                .expect("Failed to parse a year from a serial votebox");
            let year_end = years[1]
                .trim()
                .parse::<u16>()
                .map_or(year_start, |year| year);
            Year::Range(year_start, year_end)
        } else {
            match year.trim().parse::<u16>() {
                Ok(year) => Year::OneYear(year),
                Err(_) => {
                    return Err(FwErrors::InvalidYear {
                        title_id: id,
                        failed_year: year,
                    })
                }
            }
        }
    };

    let name = votebox
        .select(&Selector::parse(".preview__link").unwrap())
        .next()
        .unwrap()
        .inner_html();

    let title_url: String = format!(
        "https://filmweb.pl{}",
        votebox
            .select(&Selector::parse(".preview__link").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("href")
            .unwrap()
    );

    let alter_titles_url = format!("{title_url}/titles");
    let alter_titles = AlternateTitle::fw_get_titles(&alter_titles_url, client)?;

    let duration = {
        let document = {
            let res = client.get(&title_url).send()?.text()?;
            Html::parse_document(&res)
        };

        document
            .select(&Selector::parse(".filmCoverSection__duration").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("data-duration")
            .unwrap()
            .parse::<u16>()
            .map_or_else(
                |_| {
                    log::info!("Duration not found for {title_url}");
                    None
                },
                Some,
            )
    };
    Ok(ScrapedFwTitleData {
        id,
        year,
        name,
        url: title_url,
        alter_titles,
        duration,
    })
}

impl FwGuest {
    pub fn scrape(&self, url: &str) -> Result<Vec<FwTitle>, FwErrors> {
        let res = self.get(url).send()?.text()?;
        debug_assert!(res.contains("preview__year"));
        debug_assert!(res.contains("preview__link"));
        let document = Html::parse_document(&res);
        let mut scraped_titles = Vec::new();
        for votebox in document.select(&Selector::parse("div.myVoteBox").unwrap()) {
            let ScrapedFwTitleData {
                id,
                year,
                name,
                url,
                alter_titles,
                duration: fw_duration,
            } = scrape_from_document(votebox, &self.0)?;
            let title = FwTitle {
                id,
                year,
                name,
                url,
                alter_titles: Some(alter_titles),
                duration: fw_duration,
                imdb_data: None,
                title_type: FwTitleType::Film,
            };
            scraped_titles.push(title);
        }
        Ok(scraped_titles)
    }
}

pub fn create_imdb_client() -> Result<Client, reqwest::Error> {
    log::debug!("Creating IMDb Client");
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONNECTION,
        header::HeaderValue::from_static("keep-alive"),
    );
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip"),
    );

    Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .default_headers(headers)
        .cookie_store(true)
        .build()
}

#[derive(Debug)]
pub struct FwTitle {
    url: String,
    id: u32,
    name: String,
    alter_titles: Option<PriorityQueue<AlternateTitle, u8>>,
    title_type: FwTitleType,
    duration: Option<u16>, // in minutes
    year: Year,
    imdb_data: Option<imdb::Title>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Year {
    OneYear(u16),
    Range(u16, u16),
}

impl Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::OneYear(year) => write!(f, "{}", year),
            Self::Range(start, end) => write!(f, "{}-{}", start, end),
        }
    }
}

impl Title for FwTitle {
    fn url(&self) -> &String {
        &self.url
    }

    fn id(&self) -> u32 {
        self.id
    }

    fn title_pl(&self) -> &String {
        &self.name
    }

    fn title_type(&self) -> &FwTitleType {
        &self.title_type
    }

    fn duration(&self) -> Option<u16> {
        self.duration
    }

    fn year(&self) -> &Year {
        &self.year
    }

    fn alter_titles(&mut self) -> Option<&mut PriorityQueue<AlternateTitle, u8>> {
        self.alter_titles.as_mut()
    }

    fn imdb_data(&self) -> Option<&imdb::Title> {
        self.imdb_data.as_ref()
    }

    fn set_imdb_data_with_lookup(&mut self, client: &Client) -> Result<(), FwErrors> {
        self.imdb_data = Some(self.imdb_lookup(client)?);
        Ok(())
    }

    fn imdb_data_owned(&mut self) -> Option<imdb::Title> {
        self.imdb_data.take()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlternateTitle {
    pub language: String,
    pub title: String,
}

impl AlternateTitle {
    #[must_use]
    pub fn score_title(language: &str) -> u8 {
        if language.contains("USA") || language.contains("angielski") {
            10
        } else if language.contains("oryginalny") {
            9
        } else if language.contains("główny") {
            8
        } else if language.contains("alternatywna pisownia") {
            7
        } else if language.contains("inny tytuł") {
            6
        } else if language.contains("Polska") {
            5
        } else {
            0
        }
    }

    pub fn fw_get_titles(url: &str, client: &Client) -> Result<PriorityQueue<Self, u8>, FwErrors> {
        let response = client.get(url).send().unwrap().text()?;
        let document = Html::parse_document(&response);
        let select_titles = Selector::parse(".filmTitlesSection__title").unwrap();
        let select_language = Selector::parse(".filmTitlesSection__desc").unwrap();
        let mut titles = PriorityQueue::new();
        document
            .select(&select_titles)
            .into_iter()
            .zip(document.select(&select_language))
            .for_each(|(title, language)| {
                let title = title.inner_html();
                let language = language.inner_html();
                let score = Self::score_title(&language);
                titles.push(Self { language, title }, score);
            });
        Ok(titles)
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FwTitleType {
    Film,
    Show,
    Watchlist,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FwPageType {
    Films(u8),
    Shows(u8),
    Watchlist(u8),
}

impl FwPageType {
    fn user_url(self, username: &str) -> String {
        match self {
            Self::Films(p) => format!("https://www.filmweb.pl/user/{username}/films?page={p}"),
            Self::Shows(p) => format!(
                "https://www.filmweb.pl/user/{}/serials?page={}",
                username, p
            ),
            Self::Watchlist(p) => format!(
                "https://www.filmweb.pl/user/{}/wantToSee?page={}",
                username, p
            ),
        }
    }
}

impl From<FwPageType> for FwTitleType {
    fn from(fw_page_number: FwPageType) -> Self {
        match fw_page_number {
            FwPageType::Films(_) => Self::Film,
            FwPageType::Shows(_) => Self::Show,
            FwPageType::Watchlist(_) => Self::Watchlist,
        }
    }
}

pub fn new_client() -> Result<Client, reqwest::Error> {
    log::debug!("Creating HTTP Client");
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::CONNECTION,
        header::HeaderValue::from_static("keep-alive"),
    );
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip"),
    );

    Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .default_headers(headers)
        .cookie_store(true)
        .build()
}

pub mod authenticated {
    use crate::utils::ClientPool;

    use super::{
        imdb, scrape_from_document, AlternateTitle, Deref, FwErrors, FwPageType, FwTitle,
        FwTitleType, ScrapedFwTitleData, Title, Year, USER_AGENT,
    };
    use csv::Writer;
    use reqwest::blocking::Client;
    use reqwest::header;
    use scraper::{Html, Selector};
    use serde::{Deserialize, Serialize};
    use std::fs::File;

    #[derive(Debug)]
    pub struct FwUser {
        fw_client_pool: ClientPool,
        pub username: String,
        pub token: String,
        pub session: String,
        pub jwt: String,
        pub counts: FwUserCounts,
    }

    #[derive(Debug)]
    pub struct RatedPage {
        pub rated_titles: Vec<RatedTitle>,
    }

    #[derive(Debug)]
    pub struct ExportFiles {
        pub generic: Writer<File>,
        pub want2see: Writer<File>,
        pub favorited: Writer<File>,
    }

    impl ExportFiles {
        pub fn new() -> Result<Self, std::io::Error> {
            let write_header = |wtr| -> Writer<File> {
                let mut wtr: Writer<File> = csv::Writer::from_writer(wtr);
                wtr.write_record([
                    "Const",
                    "Your Rating",
                    "Date Rated",
                    "Title",
                    "URL",
                    "Title Type",
                    "IMDb Rating",
                    "Runtime (mins)",
                    "Year",
                    "Genres",
                    "Num Votes",
                    "Release Date",
                    "Directors",
                ])
                .unwrap();
                wtr
            };
            if let Err(e) = std::fs::create_dir("./exports") {
                match e.kind() {
                    std::io::ErrorKind::AlreadyExists => (),
                    _ => panic!("{}", e),
                }
            };
            let generic = File::create("exports/generic.csv")?;
            let want2see = File::create("exports/want2see.csv")?;
            let favorited = File::create("exports/favorited.csv")?;
            let generic = write_header(generic);
            let want2see = write_header(want2see);
            let favorited = write_header(favorited);
            Ok(Self {
                generic,
                want2see,
                favorited,
            })
        }
    }

    impl Default for ExportFiles {
        fn default() -> Self {
            Self::new().unwrap()
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct FwUserCounts {
        pub movies: u16,
        pub shows: u16,
        pub watchlist: u16,
    }

    impl FwUserCounts {
        #[must_use]
        pub const fn movies_pages(&self) -> u8 {
            (self.movies / 25 + 1) as u8
        }
        #[must_use]
        pub const fn shows_pages(&self) -> u8 {
            (self.shows / 25 + 1) as u8
        }
        #[must_use]
        pub const fn watchlist_pages(&self) -> u8 {
            (self.watchlist / 25 + 1) as u8
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct FwApiDetails {
        pub rate: u8,
        pub favorite: bool,
        #[serde(rename = "viewDate")]
        pub view_date: u32,
        pub timestamp: u128,
    }

    #[derive(Debug)]
    enum Rating {
        Rated(FwApiDetails),
        InWatchlist,
    }

    #[derive(Debug)]
    pub struct RatedTitle {
        title: FwTitle,
        rating: Rating,
    }

    impl RatedTitle {
        const fn new(title: FwTitle, rating: Rating) -> Self {
            Self { title, rating }
        }

        #[must_use]
        pub const fn is_favorited(&self) -> bool {
            match &self.rating {
                Rating::Rated(api) => api.favorite,
                Rating::InWatchlist => false,
            }
        }

        #[must_use]
        pub const fn rating(&self) -> Option<u8> {
            match &self.rating {
                Rating::Rated(api) => Some(api.rate),
                Rating::InWatchlist => None,
            }
        }

        #[must_use]
        pub fn is_duration_similar(&self, duration: u32) -> bool {
            let fw_duration = match self.duration() {
                None => return true,
                Some(duration) => duration,
            };

            let other_duration = f64::from(duration);

            // if true, it's probably a tv show, and they seem to be very different on both sites
            // so let's be less restrictive then
            let (upper, lower) = if other_duration <= 60_f64 && fw_duration <= 60_u16 {
                (other_duration * 1.50, other_duration * 0.75)
            } else {
                (other_duration * 1.15, other_duration * 0.85)
            };

            // if imdb duration doesn't fit into fw's then set it to none
            if upper >= fw_duration.into() && lower >= fw_duration.into() {
                return false;
            }
            true
        }

        pub fn to_csv_imdbv3_tmdb_files(&self, files: &mut ExportFiles) {
            let title = &self.title_pl();
            let rating = self
                .rating()
                .map_or_else(|| "WATCHLIST".to_string(), |r| r.to_string());

            let imdb_id = {
                if self.title.imdb_data.is_some() {
                    &self.title.imdb_data.as_ref().unwrap().id
                } else {
                    "not-found"
                }
            };

            // In case of year being a range, set it to the first one
            let year = match self.title.year {
                Year::OneYear(year) | Year::Range(year, _) => year.to_string(),
            };

            log::debug!(
                "Exporting to CSV title: {}, rating: {}, imdb_id: {}",
                title,
                rating,
                imdb_id
            );
            let mut fields = [""; 13];
            fields[0] = imdb_id;
            fields[1] = rating.as_ref();
            fields[3] = title.as_ref();
            fields[9] = year.as_ref();
            let write_title = |file: &mut Writer<File>| {
                file.write_record(fields).unwrap();
            };

            if self.is_favorited() {
                write_title(&mut files.favorited);
            } else {
                match &self.rating {
                    Rating::Rated(_) => write_title(&mut files.generic),
                    Rating::InWatchlist => write_title(&mut files.want2see),
                }
            }
        }
    }

    impl Title for RatedTitle {
        fn url(&self) -> &String {
            self.title.url()
        }

        fn alter_titles(
            &mut self,
        ) -> Option<&mut priority_queue::PriorityQueue<AlternateTitle, u8>> {
            self.title.alter_titles()
        }

        fn id(&self) -> u32 {
            self.title.id()
        }

        fn title_pl(&self) -> &String {
            &self.title.title_pl()
        }

        fn title_type(&self) -> &FwTitleType {
            &self.title.title_type()
        }

        fn duration(&self) -> Option<u16> {
            self.title.duration()
        }

        fn year(&self) -> &Year {
            &self.title.year()
        }
        fn set_imdb_data_with_lookup(&mut self, client: &Client) -> Result<(), FwErrors> {
            self.title.set_imdb_data_with_lookup(client)
        }

        fn imdb_data(&self) -> Option<&imdb::Title> {
            self.title.imdb_data.as_ref()
        }

        fn imdb_data_owned(&mut self) -> Option<imdb::Title> {
            self.title.imdb_data_owned()
        }
    }

    #[derive(Debug, Clone)]
    struct FwClient(Client);

    impl Deref for FwClient {
        type Target = Client;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl FwClient {
        pub fn new(token: &str, session: &str, jwt: &str) -> Self {
            let cookies = format!(
                "_fwuser_token={}; _fwuser_sessionId={}; JWT={};",
                token.trim(),
                session.trim(),
                jwt.trim()
            );

            let mut headers = header::HeaderMap::new();
            headers.insert(
                header::COOKIE,
                header::HeaderValue::from_str(&cookies).unwrap(),
            );
            headers.insert(
                header::CONNECTION,
                header::HeaderValue::from_static("keep-alive"),
            );
            headers.insert(
                header::ACCEPT_ENCODING,
                header::HeaderValue::from_static("gzip"),
            );

            Self(
                Client::builder()
                    .user_agent(USER_AGENT)
                    .gzip(true)
                    .default_headers(headers)
                    .cookie_store(true)
                    .build()
                    .unwrap(),
            )
        }

        pub fn into_client(self) -> Client {
            self.0
        }
    }

    impl FwUser {
        pub fn new(token: String, session: String, jwt: String) -> Result<Self, FwErrors> {
            let fw_client = FwClient::new(&token, &session, &jwt);
            let username = Self::get_username(&fw_client).unwrap();
            let counts = Self::rated_titles_counts(&username, &fw_client).unwrap();
            let fw_client_pool = ClientPool::new(fw_client.into_client(), 3);
            let user = Self {
                fw_client_pool,
                username,
                token,
                session,
                jwt,
                counts,
            };
            Ok(user)
        }

        pub fn scrape(&self, page: FwPageType) -> Result<RatedPage, FwErrors> {
            let mut rated_titles: Vec<_> = Vec::new();
            let url = page.user_url(&self.username);
            let res = self.fw_client_pool.get(url).send()?.text()?;

            // Ensure that these elements do exist or else it will be critical
            debug_assert!(res.contains("preview__year"));
            debug_assert!(res.contains("preview__link"));

            let document = Html::parse_document(&res);
            for votebox in document.select(&Selector::parse("div.myVoteBox").unwrap()) {
                let ScrapedFwTitleData {
                    id,
                    year,
                    name,
                    url,
                    alter_titles,
                    duration,
                } = scrape_from_document(votebox, &self.fw_client_pool)?;

                let rating: Rating = {
                    let api_response = match page {
                        FwPageType::Films(_) => Some(
                            self.fw_client_pool
                                .get(format!(
                                    "https://www.filmweb.pl/api/v1/logged/vote/film/{}/details",
                                    id
                                ))
                                .send(),
                        ),
                        FwPageType::Shows(_) => Some(
                            self.fw_client_pool
                                .get(format!(
                                    "https://www.filmweb.pl/api/v1/logged/vote/serial/{}/details",
                                    id
                                ))
                                .send(),
                        ),
                        FwPageType::Watchlist(_) => None,
                    };
                    match api_response {
                        Some(response) => match response?.json::<FwApiDetails>() {
                            Ok(v) => Rating::Rated(v),
                            Err(e) => {
                                log::info!("Bad Filmweb's api response: {e}");
                                return Err(FwErrors::InvalidJwt);
                            }
                        },
                        None => Rating::InWatchlist,
                    }
                };
                let unrated_title = FwTitle {
                    id,
                    url: url.clone(),
                    title_type: page.into(),
                    name,
                    year,
                    alter_titles: Some(alter_titles),
                    duration,
                    imdb_data: None,
                };
                rated_titles.push(RatedTitle::new(unrated_title, rating));
            }

            Ok(RatedPage { rated_titles })
        }

        fn rated_movies_count(
            username: &String,
            title_type: FwTitleType,
            fw_client: &FwClient,
        ) -> Result<u16, FwErrors> {
            let fetch = |title_type: &'static str, title_type2: &'static str| -> u16 {
                let url = format!(
                    "https://www.filmweb.pl/api/v1/user/{}/{}/{}/count",
                    username, title_type, title_type2
                );
                fw_client
                    .get(url)
                    .send()
                    .unwrap()
                    .text()
                    .unwrap()
                    .parse::<u16>()
                    .unwrap()
            };
            let count = match title_type {
                FwTitleType::Film => fetch("votes", "film"),
                FwTitleType::Show => fetch("votes", "serial"),
                FwTitleType::Watchlist => fetch("want2see", "film") + fetch("want2see", "serial"),
            };

            Ok(count)
        }

        fn rated_titles_counts(
            username: &String,
            fw_client: &FwClient,
        ) -> Result<FwUserCounts, Box<dyn std::error::Error>> {
            let movies = Self::rated_movies_count(username, FwTitleType::Film, fw_client)?;
            let shows = Self::rated_movies_count(username, FwTitleType::Show, fw_client)?;
            let watchlist = Self::rated_movies_count(username, FwTitleType::Watchlist, fw_client)?;
            Ok(FwUserCounts {
                movies,
                shows,
                watchlist,
            })
        }

        fn get_username(fw_client: &FwClient) -> Result<String, FwErrors> {
            let res = fw_client
                .get("https://www.filmweb.pl/settings")
                .send()
                .unwrap()
                .text()
                .unwrap();
            let document = Html::parse_document(&res);
            document
                .select(&Selector::parse(".mainSettings__groupItemStateContent").unwrap())
                .nth(2)
                .map_or_else(
                    || Err(FwErrors::InvalidCredentials),
                    |username_tag| Ok(username_tag.inner_html().trim().to_owned()),
                )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::authenticated::*;
    use super::*;
    use std::env;
    struct Cookies {
        token: String,
        session: String,
        jwt: String,
        username: String,
    }
    fn get_cookies() -> Cookies {
        let token = env::var("FW_TOKEN").expect("Set cookies first");
        let session = env::var("FW_SESSION").unwrap();
        let jwt = env::var("FW_JWT").unwrap();
        let username = env::var("FW_USER").unwrap();
        Cookies {
            token,
            session,
            jwt,
            username,
        }
    }

    #[test]
    fn creating_fwuser_and_username_checking_and_counts_querying() {
        let cookies = get_cookies();
        let user = FwUser::new(cookies.token, cookies.session, cookies.jwt).unwrap();
        let rated_films: Vec<RatedTitle> = user.scrape(FwPageType::Films(8)).unwrap().rated_titles;
        for film in rated_films {
            println!("{}", film.title_pl());
        }
        assert!(user.counts.movies > 0);
        assert_eq!(cookies.username, user.username);
    }
}
