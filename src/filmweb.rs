pub mod auth;
mod json;
pub mod query;
mod utils;

use crate::imdb::IMDb;
use crate::utils::create_client;
use crate::{
    imdb, AlternateTitle, AlternateTitles, FwErrors, Genre, IMDbLookup, Title, TitleID, TitleType,
    Year, USER_AGENT,
};
pub use auth::FilmwebUser;
pub use query::{Query, QueryBuilder};
use utils::{parse_my_votebox, ScrapedFwTitleData};

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

pub struct Filmweb(Client);

#[derive(Debug, Clone, FromPrimitive, Copy)]
pub enum FwGenre {
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

impl TryFrom<FwGenre> for Genre {
    type Error = ();
    // TODO: to a hashmap
    fn try_from(value: FwGenre) -> Result<Self, Self::Error> {
        match value {
            FwGenre::Action => Ok(Self::Action),
            FwGenre::AdultAnimation | FwGenre::Animation | FwGenre::Anime => Ok(Self::Animation),
            FwGenre::Adventure => Ok(Self::Adventure),
            FwGenre::Biblical
            | FwGenre::Historical
            | FwGenre::Religious
            | FwGenre::HistoricalDrama => Ok(Self::History),
            FwGenre::Fantasy => Ok(Self::Fantasy),
            FwGenre::Children
            | FwGenre::Youth
            | FwGenre::Family
            | FwGenre::Christmas
            | FwGenre::FairyTale => Ok(Self::Family),
            FwGenre::Drama
            | FwGenre::CourtroomDrama
            | FwGenre::Melodrama
            | FwGenre::Catastrophe
            | FwGenre::Grotesque => Ok(Self::Drama),
            FwGenre::Horror => Ok(Self::Horror),
            FwGenre::Crime
            | FwGenre::TrueCrime
            | FwGenre::FilmNoir
            | FwGenre::Gangster
            | FwGenre::CriminalComedy => Ok(Self::Crime),
            FwGenre::Comedy
            | FwGenre::DarkComedy
            | FwGenre::MoralComedy
            | FwGenre::RomanticComedy => Ok(Self::Comedy),
            FwGenre::Documentary
            | FwGenre::Documented
            | FwGenre::Biography
            | FwGenre::Nature
            | FwGenre::FictionalizedDocumentary => Ok(Self::Documentary),
            FwGenre::Musical | FwGenre::Musically => Ok(Self::Music),
            FwGenre::Romance => Ok(Self::Romance),
            FwGenre::SciFi => Ok(Self::SciFi),
            FwGenre::Spy | FwGenre::Surrealistic => Ok(Self::Mystery),
            FwGenre::Thriller | FwGenre::Shiver | FwGenre::Sensational => Ok(Self::Thriller),
            FwGenre::War => Ok(Self::War),
            FwGenre::Western => Ok(Self::Western),
            FwGenre::Costume
            | FwGenre::XXX
            | FwGenre::Short
            | FwGenre::Erotical
            | FwGenre::MartialArt
            | FwGenre::Poetic
            | FwGenre::Political
            | FwGenre::Propaganda
            | FwGenre::Moral
            | FwGenre::Psychological
            | FwGenre::Satire
            | FwGenre::Silent
            | FwGenre::Sports => Err(()),
        }
    }
}

lazy_static! {
    static ref STR_TO_GENRE: HashMap<&'static str, FwGenre> = {
        HashMap::from([
            ("akcja", FwGenre::Action),
            ("animacja dla dorosłych", FwGenre::AdultAnimation),
            ("animacja", FwGenre::Animation),
            ("anime", FwGenre::Anime),
            ("baśń", FwGenre::FairyTale),
            ("biblijny", FwGenre::Biblical),
            ("biograficzny", FwGenre::Biography),
            ("czarna komedia", FwGenre::DarkComedy),
            ("dla dzieci", FwGenre::Children),
            ("dla młodzieży", FwGenre::Youth),
            ("dokumentalizowany", FwGenre::Documented),
            ("dokumentalny", FwGenre::Documentary),
            ("dramat historyczny", FwGenre::HistoricalDrama),
            ("dramat obyczajowy", FwGenre::Moral),
            ("dramat sądowy", FwGenre::CourtroomDrama),
            ("dramat", FwGenre::Drama),
            ("dreszczowiec", FwGenre::Shiver),
            ("erotyczny", FwGenre::Erotical),
            ("fabularyzowany dok.", FwGenre::FictionalizedDocumentary),
            ("familijny", FwGenre::Family),
            ("fantasy", FwGenre::Fantasy),
            ("film-noir", FwGenre::FilmNoir),
            ("gangsterski", FwGenre::Gangster),
            ("groteska filmowa", FwGenre::Grotesque),
            ("historyczny", FwGenre::Historical),
            ("horror", FwGenre::Horror),
            ("katastroficzny", FwGenre::Catastrophe),
            ("komedia kryminalna", FwGenre::CriminalComedy),
            ("komedia obyczajowa", FwGenre::MoralComedy),
            ("komedia romantyczna", FwGenre::RomanticComedy),
            ("komedia rom.", FwGenre::RomanticComedy),
            ("komedia", FwGenre::Comedy),
            ("kostiumowy", FwGenre::Costume),
            ("kryminał", FwGenre::Crime),
            ("krótkometrażowy", FwGenre::Short),
            ("melodramat", FwGenre::Melodrama),
            ("musical", FwGenre::Musical),
            ("muzyczny", FwGenre::Musically),
            ("niemy", FwGenre::Silent),
            ("obyczajowy", FwGenre::Moral),
            ("poetycki", FwGenre::Poetic),
            ("politiczny", FwGenre::Political),
            ("propagandowy", FwGenre::Propaganda),
            ("przygodowy", FwGenre::Adventure),
            ("przyrodniczy", FwGenre::Nature),
            ("psychologiczny", FwGenre::Psychological),
            ("religijny", FwGenre::Religious),
            ("romans", FwGenre::Romance),
            ("satyra", FwGenre::Satire),
            ("sci-fi", FwGenre::SciFi),
            ("sensacyjny", FwGenre::Sensational),
            ("sportowy", FwGenre::Sports),
            ("surrealistyczny", FwGenre::Surrealistic),
            ("szpiegowski", FwGenre::Spy),
            ("sztuki walki", FwGenre::MartialArt),
            ("thriller", FwGenre::Thriller),
            ("true crime", FwGenre::TrueCrime),
            ("western", FwGenre::Western),
            ("wojenny", FwGenre::War),
            ("xxx", FwGenre::XXX),
            ("świąteczny", FwGenre::Christmas),
        ])
    };
}
impl From<String> for FwGenre {
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

impl Filmweb {
    /// Returns a Filmweb struct to query Filmweb
    #[must_use]
    pub fn new() -> Self {
        let http_client = create_client().expect("Can create a client");
        Self(http_client)
    }

    fn scrape_from_api(&self, api_url: &str) -> Result<Vec<FwTitle>, FwErrors> {
        log::trace!(target: "film_events", "api_url: {:?}", api_url);

        let search_results: SearchResults = {
            let res = self.get(api_url).send()?.text()?;
            serde_json::from_str(&res).unwrap()
        };

        let scraped: Vec<FwTitle> = search_results
            .search_hits
            .into_iter()
            .filter_map(|hit| {
                log::debug!(target: "film_parsing", "hit.hit_type: {:?}", &hit.hit_type);
                if let Type::Film | Type::Serial = hit.hit_type {
                    // TODO: impl Display
                    let (typ, title_type) = match hit.hit_type {
                        Type::Film => ("film", TitleType::Movie),
                        // According to the API show is a film, may change in the future
                        Type::Serial => ("film", TitleType::Show),
                        _ => panic!(),
                    };

                    let film_preview_req =
                        format!("https://www.filmweb.pl/api/v1/{typ}/{}/preview", hit.id);
                    log::trace!(target: "film_events", "film_preview_request: {:?}", film_preview_req);
                    let film_preview_res = self
                        .get(film_preview_req)
                        .send()
                        .unwrap()
                        .text()
                        .unwrap();
                    log::debug!(target: "film_api_responses", "film_preview_res: {:?}", film_preview_res);
                    let preview_result: Preview = serde_json::from_str(&film_preview_res).unwrap();
                    let year = preview_result.year;
                    let name = preview_result
                        .title
                        .map(|title| title.title)
                        .or_else(|| Some(preview_result.original_title.unwrap().title))
                        .unwrap();
                    let genres: Vec<FwGenre> = preview_result
                        .genres
                        .into_iter()
                        .map(|genre| FwGenre::from_u8(genre.id).unwrap())
                        .collect();
                    let title_url =
                        format!("https://www.filmweb.pl/{typ}/{name}-{year}-{}", hit.id,);
                    Some(FwTitle {
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
                    })
                } else {
                    None
                }
            })
            .collect();
        Ok(scraped)
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
    pub fn scrape(&self, query: &Query, page: u16) -> Result<Vec<FwTitle>, FwErrors> {
        let url = query.url(page);
        self.scrape_from_api(&url)
    }
}

/// Filmweb title struct with Title trait implemented, and other methods
#[derive(Debug)]
pub struct FwTitle {
    url: String,
    id: TitleID,
    name: String,
    fw_genres: Vec<FwGenre>,
    genres: OnceCell<Vec<Genre>>,
    alter_titles: Option<PriorityQueue<AlternateTitle, u8>>,
    title_type: TitleType,
    duration: Option<u16>, // in minutes
    year: Year,
    imdb_data: Option<imdb::IMDbTitle>,
}

impl Title for FwTitle {
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

impl AlternateTitles for FwTitle {
    fn alter_titles(&mut self) -> Option<&mut PriorityQueue<AlternateTitle, u8>> {
        self.alter_titles.as_mut()
    }
}

impl IMDbLookup for FwTitle {
    fn imdb_data(&self) -> Option<&imdb::IMDbTitle> {
        self.imdb_data.as_ref()
    }

    fn set_imdb_data_with_lookup(&mut self, imdb: &IMDb) -> Result<(), FwErrors> {
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

#[cfg(test)]
mod tests {
    use crate::filmweb::auth::{FilmwebUser, FwRatedTitle, UserPage};
    use crate::filmweb::query::QueryBuilder;
    use crate::filmweb::{Filmweb, FwGenre};
    use crate::{Title, TitleType, Year};
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
    fn scraping_filmweb() {
        let fw = Filmweb::new();
        let query = QueryBuilder::new()
            .year(Year::new(2021, 2021))
            .genres(vec![FwGenre::Comedy, FwGenre::Drama, FwGenre::SciFi])
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
        let rated_films: Vec<FwRatedTitle> =
            user.scrape(UserPage::RatedFilms(2)).unwrap().rated_titles;

        assert!(!rated_films.is_empty());
        assert!(!user.username().is_empty());
        assert!(user.counts.movies > 0);
        assert_eq!(cookies.username, user.username);
    }
}
