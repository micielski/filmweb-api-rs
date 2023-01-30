use std::fmt::Display;

use serde::{Deserialize, Serialize};

// TODO: use serde rename_all
#[derive(Serialize, Deserialize, Debug)]
pub struct SearchResults {
    pub total: u32,
    #[serde(rename = "searchCounts")]
    #[serde(skip)]
    pub search_counts: Vec<SearchCounts>,
    #[serde(rename = "searchHits")]
    pub search_hits: Vec<SearchHits>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchCounts {
    count: u16,
    #[serde(rename = "type")]
    count_type: Type,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchHits {
    pub id: u32,
    #[serde(rename = "type")]
    pub hit_type: Type,
    #[serde(rename = "matchedTitle")]
    pub matched_title: Option<String>,
    #[serde(rename = "matchedLang")]
    pub matched_lang: Option<String>,
    #[serde(rename = "filmMainCast")]
    pub film_main_cast: Option<Vec<MainCast>>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct MainCast {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Type {
    #[serde(rename = "film")]
    Film,
    #[serde(rename = "serial")]
    Serial,
    #[serde(rename = "game")]
    Game,
    #[serde(rename = "person")]
    Person,
    #[serde(rename = "news")]
    News,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "webVideo")]
    WebVideo,
    #[serde(rename = "character")]
    Character,
    #[serde(rename = "trailer")]
    Trailer,
    #[serde(rename = "review")]
    Review,
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Film => write!(f, "film"),
            Self::Serial => write!(f, "film"),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FwApiGenre {
    pub id: u8,
    #[serde(skip)] // don't touch it :)
    _name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FwApiTitle {
    pub title: String,
    pub country: String,
    pub lang: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FwApiOriginalTitle {
    pub title: String,
    country: String,
    lang: String,
    original: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(dead_code)]
pub struct Preview {
    pub year: u16,
    #[serde(rename = "entity_name")]
    #[serde(skip)]
    entity_name: String,
    #[serde(skip)]
    plot: String,
    #[serde(skip)]
    #[serde(rename = "coverPhoto")]
    cover_photo: String,
    pub title: Option<FwApiTitle>,
    #[serde(rename = "originalTitle")]
    pub original_title: Option<FwApiOriginalTitle>,
    #[serde(skip)]
    poster: String,
    pub genres: Vec<FwApiGenre>,
    pub duration: u16,
    #[serde(skip)]
    #[serde(rename = "mainReviewId")]
    main_review_id: u16,
    #[serde(skip)]
    #[serde(rename = "mainCast")]
    main_cast: MainCast,
}
