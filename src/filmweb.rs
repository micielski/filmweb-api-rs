pub mod auth;
mod json;
pub mod query;
mod utils;

use crate::error::{FilmwebScrapeError, ParseGenreError};
use crate::imdb::IMDb;
use crate::utils::create_client;
use crate::{
    imdb, AlternateTitle, AlternateTitles, FilmwebErrors, Genre, IMDbLookup, Title, TitleID,
    TitleType, Year, USER_AGENT,
};
pub use auth::FilmwebUser;
pub use query::{Query, QueryBuilder};
use utils::{parse_my_votebox, ScrapedFilmwebTitleData};

use std::collections::HashMap;
use std::ops::Deref;

use json::{Preview, SearchResults, Type};
use lazy_static::lazy_static;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use once_cell::sync::OnceCell;
use priority_queue::PriorityQueue;
use reqwest::blocking::Client;
use scraper::{Html, Selector};

/// Enum containing all genres that occur on Filmweb
#[derive(Debug, Clone, FromPrimitive, Copy)]
pub enum FilmwebGenre {
    Action = 28,                   // Akcja
    AdultAnimation = 77,           // Animacja dla dorosłych
    Adventure = 20,                // Przygodowy
    Animation = 2,                 // Animacja
    Anime = 66,                    // Anime
    Biblical = 55,                 // Biblijny
    Biography = 3,                 // Biograficzny
    Catastrophe = 40,              // Katastroficzny
    Children = 4,                  // Dla dzieci
    Christmas = 78,                // Świąteczny
    Comedy = 13,                   // Komedia
    Costume = 14,                  // Kostiumowy
    CourtroomDrama = 65,           // Dramat sądowy
    Crime = 15,                    // Kryminał
    DarkComedy = 47,               // Czarna komedia
    Documentary = 5,               // Dokumentalny
    Documented = 57,               // Dokumentalizowany
    Drama = 6,                     // Dramat
    Erotical = 7,                  // Erotyczny
    FairyTale = 42,                // Baśń
    Family = 8,                    // Familijny
    Fantasy = 9,                   // Fantasy
    FictionalizedDocumentary = 70, // Dokument fabularyzowany
    FilmNoir = 27,                 // Film-Noir
    Gangster = 53,                 // Gangsterski
    Grotesque = 60,                // Groteska filmowa
    Historical = 11,               // Historyczny
    HistoricalDrama = 59,          // Dramat Historyczny
    Horror = 12,                   // Horror
    MartialArt = 72,               // Sztuki walki
    Melodrama = 16,                // Melodramat
    Moral = 19,                    // Obyczajowy
    Musical = 17,                  // Musical
    Nature = 73,                   // Przyrodniczy
    Poetic = 62,                   // Poetycki
    Political = 43,                // Polityczny
    Propaganda = 76,               // Propagandowy
    Psychological = 38,            // Psychologiczny
    Religious = 51,                // Religijny
    Romance = 32,                  // Romans
    RomanticComedy = 30,           // Komedia romantyczna
    Satire = 39,                   // Satyra
    SciFi = 33,                    // Sci-Fi
    Sensational = 22,              // Sensacyjny
    Shiver = 46,                   // Dreszczowiec
    Short = 50,                    // Krótkometrażowy
    Silent = 67,                   // Niemy
    Sports = 61,                   // Sportowy
    Spy = 63,                      // Szpiegowski
    Surrealistic = 10,             // Surrealistyczny
    Thriller = 24,                 // Thriller
    TrueCrime = 80,                // True Crime
    War = 26,                      // Wojenny
    MoralComedy = 37,              // Komedia obyczajowa
    Western = 25,                  // Western
    XXX = 71,                      // Sex
    CriminalComedy = 58,           // Komedia kryminalna
    Musically = 44,                // Muzyczny
    Youth = 41,                    // Dla młodzieży
}

impl TryFrom<FilmwebGenre> for Genre {
    type Error = ParseGenreError;
    // TODO: to a hashmap
    fn try_from(value: FilmwebGenre) -> Result<Self, Self::Error> {
        match value {
            FilmwebGenre::Action => Ok(Self::Action),
            FilmwebGenre::AdultAnimation | FilmwebGenre::Animation | FilmwebGenre::Anime => {
                Ok(Self::Animation)
            }
            FilmwebGenre::Adventure => Ok(Self::Adventure),
            FilmwebGenre::Biblical
            | FilmwebGenre::Historical
            | FilmwebGenre::Religious
            | FilmwebGenre::HistoricalDrama => Ok(Self::History),
            FilmwebGenre::Fantasy => Ok(Self::Fantasy),
            FilmwebGenre::Children
            | FilmwebGenre::Youth
            | FilmwebGenre::Family
            | FilmwebGenre::Christmas
            | FilmwebGenre::FairyTale => Ok(Self::Family),
            FilmwebGenre::Drama
            | FilmwebGenre::CourtroomDrama
            | FilmwebGenre::Melodrama
            | FilmwebGenre::Catastrophe
            | FilmwebGenre::Grotesque => Ok(Self::Drama),
            FilmwebGenre::Horror => Ok(Self::Horror),
            FilmwebGenre::Crime
            | FilmwebGenre::TrueCrime
            | FilmwebGenre::FilmNoir
            | FilmwebGenre::Gangster
            | FilmwebGenre::CriminalComedy => Ok(Self::Crime),
            FilmwebGenre::Comedy
            | FilmwebGenre::DarkComedy
            | FilmwebGenre::MoralComedy
            | FilmwebGenre::RomanticComedy => Ok(Self::Comedy),
            FilmwebGenre::Documentary
            | FilmwebGenre::Documented
            | FilmwebGenre::Biography
            | FilmwebGenre::Nature
            | FilmwebGenre::FictionalizedDocumentary => Ok(Self::Documentary),
            FilmwebGenre::Musical | FilmwebGenre::Musically => Ok(Self::Music),
            FilmwebGenre::Romance => Ok(Self::Romance),
            FilmwebGenre::SciFi => Ok(Self::SciFi),
            FilmwebGenre::Spy | FilmwebGenre::Surrealistic => Ok(Self::Mystery),
            FilmwebGenre::Thriller | FilmwebGenre::Shiver | FilmwebGenre::Sensational => {
                Ok(Self::Thriller)
            }
            FilmwebGenre::War => Ok(Self::War),
            FilmwebGenre::Western => Ok(Self::Western),
            FilmwebGenre::Costume
            | FilmwebGenre::XXX
            | FilmwebGenre::Short
            | FilmwebGenre::Erotical
            | FilmwebGenre::MartialArt
            | FilmwebGenre::Poetic
            | FilmwebGenre::Political
            | FilmwebGenre::Propaganda
            | FilmwebGenre::Moral
            | FilmwebGenre::Psychological
            | FilmwebGenre::Satire
            | FilmwebGenre::Silent
            | FilmwebGenre::Sports => Err(ParseGenreError),
        }
    }
}

lazy_static! {
    static ref STR_TO_GENRE: HashMap<&'static str, FilmwebGenre> = {
        HashMap::from([
            ("akcja", FilmwebGenre::Action),
            ("animacja dla dorosłych", FilmwebGenre::AdultAnimation),
            ("animacja", FilmwebGenre::Animation),
            ("anime", FilmwebGenre::Anime),
            ("baśń", FilmwebGenre::FairyTale),
            ("biblijny", FilmwebGenre::Biblical),
            ("biograficzny", FilmwebGenre::Biography),
            ("czarna komedia", FilmwebGenre::DarkComedy),
            ("dla dzieci", FilmwebGenre::Children),
            ("dla młodzieży", FilmwebGenre::Youth),
            ("dokumentalizowany", FilmwebGenre::Documented),
            ("dokumentalny", FilmwebGenre::Documentary),
            ("dramat historyczny", FilmwebGenre::HistoricalDrama),
            ("dramat obyczajowy", FilmwebGenre::Moral),
            ("dramat sądowy", FilmwebGenre::CourtroomDrama),
            ("dramat", FilmwebGenre::Drama),
            ("dreszczowiec", FilmwebGenre::Shiver),
            ("erotyczny", FilmwebGenre::Erotical),
            (
                "fabularyzowany dok.",
                FilmwebGenre::FictionalizedDocumentary,
            ),
            ("familijny", FilmwebGenre::Family),
            ("fantasy", FilmwebGenre::Fantasy),
            ("film-noir", FilmwebGenre::FilmNoir),
            ("gangsterski", FilmwebGenre::Gangster),
            ("groteska filmowa", FilmwebGenre::Grotesque),
            ("historyczny", FilmwebGenre::Historical),
            ("horror", FilmwebGenre::Horror),
            ("katastroficzny", FilmwebGenre::Catastrophe),
            ("komedia kryminalna", FilmwebGenre::CriminalComedy),
            ("komedia obyczajowa", FilmwebGenre::MoralComedy),
            ("komedia obycz.", FilmwebGenre::MoralComedy),
            ("komedia romantyczna", FilmwebGenre::RomanticComedy),
            ("komedia rom.", FilmwebGenre::RomanticComedy),
            ("komedia", FilmwebGenre::Comedy),
            ("kostiumowy", FilmwebGenre::Costume),
            ("kryminał", FilmwebGenre::Crime),
            ("krótkometrażowy", FilmwebGenre::Short),
            ("melodramat", FilmwebGenre::Melodrama),
            ("musical", FilmwebGenre::Musical),
            ("muzyczny", FilmwebGenre::Musically),
            ("niemy", FilmwebGenre::Silent),
            ("obyczajowy", FilmwebGenre::Moral),
            ("poetycki", FilmwebGenre::Poetic),
            ("politiczny", FilmwebGenre::Political),
            ("propagandowy", FilmwebGenre::Propaganda),
            ("przygodowy", FilmwebGenre::Adventure),
            ("przyrodniczy", FilmwebGenre::Nature),
            ("psychologiczny", FilmwebGenre::Psychological),
            ("religijny", FilmwebGenre::Religious),
            ("romans", FilmwebGenre::Romance),
            ("satyra", FilmwebGenre::Satire),
            ("sci-fi", FilmwebGenre::SciFi),
            ("sensacyjny", FilmwebGenre::Sensational),
            ("sportowy", FilmwebGenre::Sports),
            ("surrealistyczny", FilmwebGenre::Surrealistic),
            ("szpiegowski", FilmwebGenre::Spy),
            ("sztuki walki", FilmwebGenre::MartialArt),
            ("thriller", FilmwebGenre::Thriller),
            ("true crime", FilmwebGenre::TrueCrime),
            ("western", FilmwebGenre::Western),
            ("wojenny", FilmwebGenre::War),
            ("xxx", FilmwebGenre::XXX),
            ("świąteczny", FilmwebGenre::Christmas),
        ])
    };
}
impl From<String> for FilmwebGenre {
    fn from(value: String) -> Self {
        STR_TO_GENRE[value.trim().to_lowercase().as_str()]
    }
}

impl Deref for Filmweb {
    type Target = Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Filmweb {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct containing methods to query Filmweb
pub struct Filmweb(Client);

impl Filmweb {
    /// Returns a Filmweb struct to query Filmweb
    #[must_use]
    pub fn new() -> Self {
        let http_client = create_client().expect("Can create a client");
        Self(http_client)
    }

    fn scrape_from_api(&self, api_url: &str) -> Result<Vec<FilmwebTitle>, FilmwebScrapeError> {
        log::trace!(target: "film_events", "api_url: {:?}", api_url);

        let mut found_titles: Vec<FilmwebTitle> = Vec::new();
        let search_results: SearchResults = {
            let res = self.get(api_url).send()?.text()?;
            serde_json::from_str(&res).unwrap()
        };

        for hit in search_results.search_hits {
            if let Type::Film | Type::Serial = hit.hit_type {
                let (title_type_str, title_type) = match hit.hit_type {
                    Type::Film => ("film", TitleType::Movie),
                    Type::Serial => ("film", TitleType::Show),
                    _ => panic!("Shouldn't be possible"),
                };

                let film_preview_req_url = format!(
                    "https://www.filmweb.pl/api/v1/{title_type_str}/{}/preview",
                    hit.id
                );
                let film_preview_res = self.get(film_preview_req_url).send()?.text()?;
                let preview_result: Preview = serde_json::from_str(&film_preview_res)?;
                let year = preview_result.year;
                let name = preview_result
                    .title
                    .map(|title| title.title)
                    .or_else(|| Some(preview_result.original_title.unwrap().title))
                    .expect("it'll always be some");
                let genres: Vec<FilmwebGenre> = preview_result
                    .genres
                    .into_iter()
                    .map(|genre| FilmwebGenre::from_u8(genre.id).unwrap())
                    .collect();
                let title_url = format!(
                    "https://www.filmweb.pl/{title_type_str}/{name}-{year}-{}",
                    hit.id
                );
                let title = FilmwebTitle {
                    alter_titles: AlternateTitle::fw_get_titles(&title_url, &self.0).ok(),
                    name,
                    fw_genres: genres,
                    genres: OnceCell::new(),
                    id: TitleID::FilmwebID(hit.id),
                    year: year.into(),
                    duration: Some(preview_result.duration),
                    title_type,
                    imdb_data: None,
                    url: title_url,
                };
                found_titles.push(title);
            }
        }
        Ok(found_titles)
    }

    /// Scrapes Filmweb's database with a given query
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use filmed::filmweb::{Filmweb, FwGenre, QueryBuilder};
    /// use filmed::{Title, Year};
    ///
    /// let fw = Filmweb::new();
    /// let query = QueryBuilder::new()
    ///     .year(Year::from_str("2017-2021"))
    ///     .genres(vec![FwGenre::Drama, FwGenre::Romance])
    ///     .build();
    /// // Scrapes a page containing most often rated comedies, 1 is the page number
    /// // While Joker may not be a romance, it's still a drama
    /// let results = fw.scrape(&query, 1)?;
    /// assert_eq!(results[0].title(), "Joker");
    /// assert_eq!(results[0].year(), Year::from_str("2019"));
    /// #
    /// #    Ok(())
    /// # }
    /// ```
    pub fn scrape(
        &self,
        query: &Query,
        page: u16,
    ) -> Result<Vec<FilmwebTitle>, FilmwebScrapeError> {
        let url = query.url(page);
        self.scrape_from_api(&url)
    }
}

/// Filmweb title struct with Title trait implemented, and other methods
#[derive(Debug)]
pub struct FilmwebTitle {
    url: String,
    id: TitleID,
    name: String,
    fw_genres: Vec<FilmwebGenre>,
    genres: OnceCell<Vec<Genre>>,
    alter_titles: Option<PriorityQueue<AlternateTitle, u8>>,
    title_type: TitleType,
    duration: Option<u16>, // in minutes
    year: Year,
    imdb_data: Option<imdb::IMDbTitle>,
}

impl Title for FilmwebTitle {
    fn url(&self) -> &String {
        &self.url
    }

    fn id(&self) -> &TitleID {
        &self.id
    }

    fn title(&self) -> &String {
        &self.name
    }

    fn title_type(&self) -> &TitleType {
        &self.title_type
    }

    fn duration(&self) -> Option<u16> {
        self.duration
    }

    fn genres(&self) -> &Vec<Genre> {
        // TODO: optimize this
        if self.genres.get().is_none() {
            self.genres
                .set(
                    self.fw_genres
                        .iter()
                        .filter_map(|fwgenre| Genre::try_from(*fwgenre).ok())
                        .collect(),
                )
                .unwrap();
        };
        self.genres.get().unwrap()
    }

    fn year(&self) -> Year {
        self.year
    }
}

impl AlternateTitles for FilmwebTitle {
    fn alter_titles(&mut self) -> Option<&mut PriorityQueue<AlternateTitle, u8>> {
        self.alter_titles.as_mut()
    }
}

impl IMDbLookup for FilmwebTitle {
    fn imdb_data(&self) -> Option<&imdb::IMDbTitle> {
        self.imdb_data.as_ref()
    }

    fn set_imdb_data_with_lookup(&mut self, imdb: &IMDb) -> Result<(), FilmwebErrors> {
        self.imdb_data = Some(self.imdb_lookup(imdb)?);
        Ok(())
    }

    fn imdb_data_owned(&mut self) -> Option<imdb::IMDbTitle> {
        self.imdb_data.take()
    }
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

    pub fn fw_get_titles(
        url: &str,
        client: &Client,
    ) -> Result<PriorityQueue<Self, u8>, FilmwebErrors> {
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

#[cfg(test)]
mod tests {
    use crate::filmweb::auth::{FilmwebRatedTitle, FilmwebUser, UserPage};
    use crate::filmweb::query::QueryBuilder;
    use crate::filmweb::{Filmweb, FilmwebGenre};
    use crate::{Title, TitleType, User, Year};
    use std::env;

    struct Cookies {
        token: String,
        session: String,
        jwt: String,
        username: String,
    }

    fn get_cookies() -> Cookies {
        let token = env::var("FW_TOKEN").expect("cookies are set via env");
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
    fn scraping_filmweb() {
        let fw = Filmweb::new();
        let query = QueryBuilder::new()
            .year(Year::new(2021, 2021))
            .genres(vec![
                FilmwebGenre::Comedy,
                FilmwebGenre::Drama,
                FilmwebGenre::SciFi,
            ])
            .build();
        let fw_search_result = fw.scrape(&query, 1).unwrap();

        assert_eq!(fw_search_result[0].title(), "Diuna");
        assert_eq!(fw_search_result[0].title_type(), &TitleType::Movie);
        let year: u16 = fw_search_result[0].year().into();
        assert_eq!(year, 2021);
    }

    #[test]
    fn creating_fwuser_and_username_checking_and_counts_querying() {
        let cookies = get_cookies();
        let user = FilmwebUser::new(cookies.token, cookies.session, cookies.jwt).unwrap();
        let rated_films: Vec<FilmwebRatedTitle> =
            user.scrape(UserPage::RatedFilms(2)).unwrap().rated_titles;

        assert!(!rated_films.is_empty());
        assert!(!user.username().is_empty());
        assert!(user.num_of_rated_movies() > 0);
        assert_eq!(cookies.username, *user.username());
    }
}
