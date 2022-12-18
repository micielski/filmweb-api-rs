pub mod error;
pub use error::FwErrors;
use priority_queue::PriorityQueue;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:106.0) Gecko/20100101 Firefox/106.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FwPage {
    pub page: FwPageNumbered,
    pub rated_titles: Vec<FwTitle>,
}

trait Title {
    fn fw_url(&self) -> &String;

    fn fw_id(&self) -> u32;

    fn fw_title_pl(&self) -> &String;

    fn title_type(&self) -> &FwTitleType;

    fn fw_duration(&self) -> Option<u16>;

    fn year(&self) -> &Year;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FwTitle {
    fw_url: String,
    fw_id: u32,
    fw_title_pl: String,
    fw_alter_titles: Option<PriorityQueue<AlternateTitle, u8>>,
    title_type: FwTitleType,
    fw_duration: Option<u16>, // time in minutes
    year: Year,
    imdb_data: Option<IMDbApiDetails>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Year {
    OneYear(u16),
    Range(u16, u16),
}

impl FwTitle {
    pub fn fw_url(&self) -> &String {
        &self.fw_url
    }

    pub fn fw_id(&self) -> u32 {
        self.fw_id
    }

    pub fn fw_title_pl(&self) -> &String {
        &self.fw_title_pl
    }

    pub fn title_type(&self) -> &FwTitleType {
        &self.title_type
    }

    pub fn fw_duration(&self) -> Option<u16> {
        self.fw_duration
    }

    pub fn year(&self) -> &Year {
        &self.year
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct IMDbApiDetails {
    pub title: String,
    pub id: String,
    pub duration: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlternateTitle {
    pub language: String,
    pub title: String,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FwTitleType {
    Film,
    Serial,
    WantsToSee,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FwPageNumbered {
    Films(u8),
    Serials(u8),
    WantsToSee(u8),
}

impl From<FwPageNumbered> for FwTitleType {
    fn from(fw_page_number: FwPageNumbered) -> Self {
        match fw_page_number {
            FwPageNumbered::Films(_) => Self::Film,
            FwPageNumbered::Serials(_) => Self::Serial,
            FwPageNumbered::WantsToSee(_) => Self::WantsToSee,
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
    use super::{FwErrors, FwTitle, FwTitleType, Title, Year, USER_AGENT};
    use priority_queue::PriorityQueue;
    use reqwest::blocking::Client;
    use reqwest::header;
    use scraper::{Html, Selector};
    use serde::{Deserialize, Serialize};
    use std::ops::Deref;

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct FwUser {
        pub username: String,
        pub token: String,
        pub session: String,
        pub jwt: String,
        pub counts: UserCounts,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct UserCounts {
        pub movies: u16,
        pub shows: u16,
        pub marked_to_see: u16,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct FwApiDetails {
        pub rate: u8,
        pub favorite: bool,
        #[serde(rename = "viewDate")]
        pub view_date: u32,
        pub timestamp: u128,
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct IMDbApiDetails {
        pub title: String,
        pub id: String,
        pub duration: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FwRatedTitle {
        title: FwTitle,
        rating: FwApiDetails,
    }

    impl FwRatedTitle {
        pub fn new(title: FwTitle, rating: FwApiDetails) -> Self {
            FwRatedTitle { title, rating }
        }

        pub fn is_favorited(&self) -> bool {
            self.rating.favorite
        }

        pub fn rating(&self) -> u8 {
            self.rating.rate
        }
    }

    impl Title for FwRatedTitle {
        fn fw_url(&self) -> &String {
            &self.title.fw_url
        }

        fn fw_id(&self) -> u32 {
            self.title.fw_id
        }

        fn fw_title_pl(&self) -> &String {
            &self.title.fw_title_pl
        }

        fn title_type(&self) -> &FwTitleType {
            &self.title.title_type
        }

        fn fw_duration(&self) -> Option<u16> {
            self.title.fw_duration
        }

        fn year(&self) -> &Year {
            &self.title.year
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
    pub struct AlternateTitle {
        pub language: String,
        pub title: String,
    }

    pub struct FwClient(Client);

    impl Deref for FwClient {
        type Target = Client;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl FwClient {
        pub fn new(token: String, session: String, jwt: String) -> Self {
            let cookies = format!(
                "_fwuser_token={}; _fwuser_sessionId={}; JWT{};",
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

            FwClient(
                Client::builder()
                    .user_agent(USER_AGENT)
                    .gzip(true)
                    .default_headers(headers)
                    .cookie_store(true)
                    .build()
                    .unwrap(),
            )
        }
    }

    impl FwUser {
        #[must_use]
        pub fn new(
            token: String,
            session: String,
            jwt: String,
        ) -> Result<(Self, FwClient), FwErrors> {
            let fw_client = FwClient::new(token.clone(), session.clone(), jwt.clone());
            let username = Self::get_username(&fw_client).unwrap();
            let counts = Self::get_counts(&username, &fw_client).unwrap();
            let user = Self {
                username,
                token,
                session,
                jwt,
                counts,
            };
            Ok((user, fw_client))
        }

        fn get_count(
            username: &String,
            title_type: FwTitleType,
            fw_client: FwClient,
        ) -> Result<u16, FwErrors> {
            let fetch = |title_type: &'static str, title_type2: &'static str| -> u16 {
                fw_client
                    .get(format!(
                        "https://www.filmweb.pl/api/v1/user/{}/{}/{}/count",
                        username, title_type2, title_type
                    ))
                    .send()
                    .unwrap()
                    .text()
                    .unwrap()
                    .parse::<u16>()
                    .unwrap()
            };
            let count = match title_type {
                FwTitleType::Film => fetch("votes", "film"),
                FwTitleType::Serial => fetch("votes", "serial"),
                FwTitleType::WantsToSee => fetch("want2see", "film"),
            };

            Ok(count)
        }

        fn get_counts(
            username: &String,
            fw_client: &Client,
        ) -> Result<UserCounts, Box<dyn std::error::Error>> {
            let movies: u16 = fw_client
                .get(format!(
                    "https://www.filmweb.pl/api/v1/user/{}/votes/film/count",
                    username
                ))
                .send()?
                .text()?
                .parse()
                .unwrap();

            let marked_to_see_movies: u16 = fw_client
                .get(format!(
                    "https://www.filmweb.pl/api/v1/user/{}/want2see/film/count",
                    username
                ))
                .send()?
                .text()?
                .parse()
                .unwrap();

            let shows: u16 = fw_client
                .get(format!(
                    "https://www.filmweb.pl/api/v1/user/{}/votes/serial/count",
                    username
                ))
                .send()?
                .text()?
                .parse()
                .unwrap();

            let marked_to_see_shows: u16 = fw_client
                .get(format!(
                    "https://www.filmweb.pl/api/v1/user/{}/want2see/serial/count",
                    username
                ))
                .send()?
                .text()?
                .parse()
                .unwrap();
            let marked_to_see = marked_to_see_shows + marked_to_see_movies;

            Ok(UserCounts {
                movies,
                shows,
                marked_to_see,
            })
        }

        pub fn get_username(fw_client: &FwClient) -> Result<String, FwErrors> {
            let res = fw_client
                .get("https://www.filmweb.pl/settings")
                .send()
                .unwrap()
                .text()
                .unwrap();
            let document = Html::parse_document(&res);
            match document
                .select(&Selector::parse(".mainSettings__groupItemStateContent").unwrap())
                .nth(2)
            {
                Some(username_tag) => return Ok(username_tag.inner_html().trim().to_owned()),
                None => return Err(FwErrors::InvalidCredentials),
            };
        }
    }
}
