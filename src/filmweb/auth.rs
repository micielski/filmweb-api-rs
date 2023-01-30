/// Module containing logged-in user related things.
use crate::{imdb::IMDb, utils::ClientPool, AlternateTitles, IMDbLookup, RatedTitle, TitleID};

use super::{
    imdb, parse_my_votebox, AlternateTitle, Deref, FwErrors, FwTitle, ScrapedFwTitleData, Title,
    TitleType, Year, USER_AGENT,
};
use csv::Writer;
use once_cell::sync::OnceCell;
use reqwest::blocking::Client;
use reqwest::header;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs::File;

#[derive(Debug)]
pub struct FilmwebUser {
    fw_client_pool: ClientPool,
    pub username: String,
    pub token: String,
    pub session: String,
    pub jwt: String,
    pub counts: FwUserCounts,
}

#[derive(Debug)]
pub struct RatedPage {
    pub rated_titles: Vec<FwRatedTitle>,
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

/// Enum that defines a url of rated titles or watchlisted titles.
/// RatedFilms(2) would be filmweb.pl/user/{USERNAME}/films?page=2
/// RatedShows(4) filmweb.pl/user/{USERNAME}/serials?page=4
/// Watchlist(6) filmweb.pl/user/{USERNAME}/wantToSee?page=6
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UserPage {
    RatedFilms(u8),
    RatedShows(u8),
    Watchlist(u8),
}

/// Enum that defines type of a user page.
/// `RatedFilms` would be filmweb.pl/user/{USERNAME}/films
/// `RatedShows` filmweb.pl/user/{USERNAME}/serials
/// `Watchlist` filmweb.pl/user/{USERNAME}/wantToSee
#[derive(Copy, Clone, Deserialize, Serialize, Debug, PartialEq, Eq, Hash)]
pub enum UserPageType {
    RatedFilms,
    RatedShows,
    Watchlist,
}

impl UserPage {
    ///
    fn user_url(self, username: &str) -> String {
        match self {
            Self::RatedFilms(p) => {
                format!("https://www.filmweb.pl/user/{username}/films?page={p}")
            }
            Self::RatedShows(p) => format!(
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

impl From<UserPage> for UserPageType {
    fn from(fw_page_number: UserPage) -> Self {
        match fw_page_number {
            UserPage::RatedFilms(_) => Self::RatedFilms,
            UserPage::RatedShows(_) => Self::RatedShows,
            UserPage::Watchlist(_) => Self::Watchlist,
        }
    }
}
#[derive(Debug)]
pub struct FwRatedTitle {
    title: FwTitle,
    rating: Option<u8>,
    is_favorited: bool,
    is_watchlisted: bool,
}

impl RatedTitle for FwRatedTitle {
    fn rating(&self) -> Option<u8> {
        self.rating
    }

    fn is_favorited(&self) -> bool {
        self.is_favorited
    }

    fn is_watchlisted(&self) -> bool {
        self.is_watchlisted
    }
}

impl FwRatedTitle {
    const fn new(title: FwTitle, rating: Option<u8>, favorited: bool, watchlisted: bool) -> Self {
        Self {
            title,
            rating,
            is_favorited: favorited,
            is_watchlisted: watchlisted,
        }
    }

    pub fn to_csv_imdbv3_tmdb_files(&self, files: &mut ExportFiles) {
        let title = &self.title();
        let rating = self
            .rating()
            .map_or_else(|| "WATCHLIST".to_string(), |r| r.to_string());

        let not_found = String::from("not-found");
        let imdb_id = match self.title.imdb_data().unwrap().id() {
            TitleID::IMDbID(id) => id,
            _ => &not_found,
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

        match (self.is_favorited(), self.is_watchlisted(), self.rating()) {
            (true, false, Some(_)) => write_title(&mut files.favorited),
            (false, true, None) => write_title(&mut files.want2see),
            (false, false, Some(_)) => write_title(&mut files.generic),
            _ => panic!("It can't be possible"),
        }
    }
}

impl Title for FwRatedTitle {
    fn url(&self) -> &String {
        self.title.url()
    }

    fn id(&self) -> &TitleID {
        self.title.id()
    }

    fn title(&self) -> &String {
        self.title.title()
    }

    fn title_type(&self) -> &TitleType {
        self.title.title_type()
    }

    fn duration(&self) -> Option<u16> {
        self.title.duration()
    }

    fn year(&self) -> Year {
        self.title.year()
    }

    fn genres(&self) -> &Vec<crate::Genre> {
        self.title.genres()
    }
}

impl AlternateTitles for FwRatedTitle {
    fn alter_titles(&mut self) -> Option<&mut priority_queue::PriorityQueue<AlternateTitle, u8>> {
        self.title.alter_titles()
    }
}

impl IMDbLookup for FwRatedTitle {
    fn set_imdb_data_with_lookup(&mut self, imdb: &IMDb) -> Result<(), FwErrors> {
        self.title.set_imdb_data_with_lookup(imdb)
    }

    fn imdb_data(&self) -> Option<&imdb::IMDbTitle> {
        self.title.imdb_data.as_ref()
    }

    fn imdb_data_owned(&mut self) -> Option<imdb::IMDbTitle> {
        self.title.imdb_data_owned()
    }
}

/// Reqwest client but with JWT,
#[derive(Debug, Clone)]
struct FwUserHttpClient(Client);

impl Deref for FwUserHttpClient {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FwUserHttpClient {
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

impl FilmwebUser {
    pub fn new<T: ToString>(token: T, session: T, jwt: T) -> Result<Self, FwErrors> {
        let token = token.to_string();
        let session = session.to_string();
        let jwt = jwt.to_string();
        let fw_client = FwUserHttpClient::new(&token, &session, &jwt);
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

    pub fn scrape(&self, page: UserPage) -> Result<RatedPage, FwErrors> {
        let mut rated_titles: Vec<_> = Vec::new();
        let url = page.user_url(&self.username);
        let res = self.fw_client_pool.get(url).send()?.text()?;

        // Ensure that these elements do exist or else it will be critical
        debug_assert!(res.contains("preview__link"));

        let document = Html::parse_document(&res);
        for votebox in document.select(&Selector::parse("div.myVoteBox").unwrap()) {
            let ScrapedFwTitleData {
                id,
                year,
                genres: fw_genres,
                name,
                url,
                alter_titles,
                duration,
            } = parse_my_votebox(votebox, &self.fw_client_pool)?;

            let title_type = match page {
                UserPage::RatedFilms(_) => TitleType::Movie,
                UserPage::RatedShows(_) => TitleType::Show,
                UserPage::Watchlist(_) => {
                    if url.contains(".pl/serial/") {
                        TitleType::Show
                    } else {
                        TitleType::Movie
                    }
                }
            };

            let (rating, is_favorited, is_watchlisted) = {
                let api_response = match page {
                    UserPage::RatedFilms(_) => Some(
                        self.fw_client_pool
                            .get(format!(
                                "https://www.filmweb.pl/api/v1/logged/vote/film/{}/details",
                                id
                            ))
                            .send(),
                    ),
                    UserPage::RatedShows(_) => Some(
                        self.fw_client_pool
                            .get(format!(
                                "https://www.filmweb.pl/api/v1/logged/vote/serial/{}/details",
                                id
                            ))
                            .send(),
                    ),
                    UserPage::Watchlist(_) => None,
                };
                match api_response {
                    Some(response) => match response?.json::<FwApiDetails>() {
                        Ok(v) => (Some(v.rate), v.favorite, false),
                        Err(e) => {
                            log::info!("Bad Filmweb's api response: {e}");
                            return Err(FwErrors::InvalidJwt);
                        }
                    },
                    None => (None, false, true),
                }
            };

            let unrated_title = FwTitle {
                id: TitleID::FilmwebID(id),
                url: url.clone(),
                title_type,
                fw_genres,
                genres: OnceCell::new(),
                name,
                year,
                alter_titles: Some(alter_titles),
                duration,
                imdb_data: None,
            };

            rated_titles.push(FwRatedTitle::new(
                unrated_title,
                rating,
                is_favorited,
                is_watchlisted,
            ));
        }

        Ok(RatedPage { rated_titles })
    }

    fn rated_movies_count(
        username: &String,
        title_type: UserPageType,
        fw_client: &FwUserHttpClient,
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
            UserPageType::RatedFilms => fetch("votes", "film"),
            UserPageType::RatedShows => fetch("votes", "serial"),
            UserPageType::Watchlist => fetch("want2see", "film") + fetch("want2see", "serial"),
        };

        Ok(count)
    }

    fn rated_titles_counts(
        username: &String,
        fw_client: &FwUserHttpClient,
    ) -> Result<FwUserCounts, Box<dyn std::error::Error>> {
        let movies = Self::rated_movies_count(username, UserPageType::RatedFilms, fw_client)?;
        let shows = Self::rated_movies_count(username, UserPageType::RatedShows, fw_client)?;
        let watchlist = Self::rated_movies_count(username, UserPageType::Watchlist, fw_client)?;
        Ok(FwUserCounts {
            movies,
            shows,
            watchlist,
        })
    }

    /// Returns user's username
    ///
    /// # Examples
    ///
    /// ```should_panic
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use filmed::filmweb::FilmwebUser;
    /// let user = FilmwebUser::new("FW_TOKEN", "FW_SESSION", "JWT")?;
    /// let username = user.username();
    /// assert_eq!(username, "your username");
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    #[must_use]
    pub const fn username(&self) -> &String {
        &self.username
    }

    fn get_username(fw_client: &FwUserHttpClient) -> Result<String, FwErrors> {
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
