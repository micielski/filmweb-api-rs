//! # filmweb-api
//!
//! Unofficial API to query and interact with filmweb.pl from Rust with support for authentication
//! with cookis.
//! Highly prone to breaking changes.

/// Error types
pub mod error;

/// `Filmweb` api
pub mod filmweb;

/// `IMDb` api
pub mod imdb;

mod utils;

use std::fmt::{self, Display};

use error::FwErrors;
use imdb::IMDb;
use priority_queue::PriorityQueue;
use serde::{Deserialize, Serialize};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:106.0) Gecko/20100101 Firefox/108.0";

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TitleID {
    FilmwebID(u32),
    IMDbID(String),
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Year {
    OneYear(u16),
    Range(u16, u16),
}

impl Year {
    #[must_use]
    pub const fn new(first: u16, second: u16) -> Self {
        if first == second {
            Self::OneYear(first)
        } else {
            Self::Range(first, second)
        }
    }

    #[must_use]
    pub const fn start(self) -> u16 {
        match self {
            Self::OneYear(year) => year,
            Self::Range(start, _) => start,
        }
    }

    #[must_use]
    pub const fn end(self) -> u16 {
        match self {
            Self::OneYear(year) => year,
            Self::Range(_, end) => end,
        }
    }

    /// Converts str to a year
    ///
    /// # Examples
    /// ```
    /// use filmed::Year;
    /// let year1 = Year::from_str("(2015-2017)");
    /// let year2 = Year::from_str("1984-2021");
    /// let year3 = Year::from_str("2040");
    /// assert_eq!((year1.start(), year1.end()), (2015, 2017));
    /// assert_eq!((year2.start(), year2.end()), (1984, 2021));
    /// assert_eq!(year3.start(), 2040);
    /// ```
    #[must_use]
    pub fn from_str(year: &str) -> Year {
        let dirty_year = year.trim_start_matches('(').trim_end_matches(')');
        if dirty_year.contains('-') {
            let after_split: Vec<&str> = dirty_year.split('-').collect();
            Year::Range(
                after_split[0].parse::<u16>().expect("it's a year"),
                after_split[1].parse::<u16>().expect("it's a year"),
            )
        } else {
            Year::OneYear(dirty_year.parse::<u16>().expect("it's a year"))
        }
    }
}

impl From<u16> for Year {
    fn from(value: u16) -> Self {
        Self::OneYear(value)
    }
}

impl From<Year> for u16 {
    fn from(value: Year) -> Self {
        match value {
            Year::OneYear(year) | Year::Range(year, _) => year,
        }
    }
}

impl Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::OneYear(year) => write!(f, "{year}"),
            Self::Range(start, end) => write!(f, "{start}-{end}"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TitleType {
    Movie,
    Show,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Genre {
    Action,
    Adventure,
    Animation,
    Comedy,
    Crime,
    Documentary,
    Drama,
    Family,
    Fantasy,
    History,
    Horror,
    Music,
    Mystery,
    Romance,
    SciFi,
    Thriller,
    War,
    Western,
}

impl TryFrom<&str> for Genre {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_ascii_lowercase().as_str() {
            "action" => Ok(Self::Action),
            "adventure" => Ok(Self::Adventure),
            "animation" => Ok(Self::Animation),
            "comedy" => Ok(Self::Comedy),
            "crime" => Ok(Self::Crime),
            "documentary" => Ok(Self::Documentary),
            "drama" => Ok(Self::Drama),
            "family" => Ok(Self::Family),
            "fantasy" => Ok(Self::Fantasy),
            "history" => Ok(Self::History),
            "horror" => Ok(Self::Horror),
            "music" => Ok(Self::Music),
            "mystery" => Ok(Self::Mystery),
            "romance" => Ok(Self::Romance),
            "sci-fi" => Ok(Self::SciFi),
            "thriller" => Ok(Self::Thriller),
            "war" => Ok(Self::War),
            "western" => Ok(Self::Western),
            _ => Err(()),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlternateTitle {
    pub language: String,
    pub title: String,
}

pub trait Title {
    fn url(&self) -> &String;

    fn id(&self) -> &TitleID;

    fn title(&self) -> &String;

    fn title_type(&self) -> &TitleType;

    fn duration(&self) -> Option<u16>;

    fn genres(&self) -> &Vec<Genre>;

    fn year(&self) -> Year;

    fn compare_year(&self, other_year: Year) -> bool {
        let year_l = self.year().start();
        let year_r = other_year.start();
        year_l == year_r || year_l == year_r + 1 || year_l == year_r - 1
    }

    /// Compare title's duration with another. Returns true if they're similar enough to be the
    /// same title. It's a little bit mild if one of the durations is short, as it may be that
    /// it's a TV show and they tend to differ very between movie tracking platforms
    /// Returns true if title's duration is none
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
    ///     .build();
    /// let results = fw.scrape(&query, 1)?;
    /// assert!(results[0].is_duration_similar(150));
    /// #
    /// #     Ok(())
    /// # }
    /// ````
    #[must_use]
    fn is_duration_similar(&self, duration: u32) -> bool {
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

        // if duration doesn't fit into other duration return false
        if upper >= fw_duration.into() && lower >= fw_duration.into() {
            return false;
        }
        true
    }
}

pub trait AlternateTitles: Title {
    fn alter_titles(&mut self) -> Option<&mut PriorityQueue<AlternateTitle, u8>>;
}

pub trait IMDbLookup: Title + AlternateTitles {
    fn set_imdb_data_with_lookup(&mut self, imdb: &IMDb) -> Result<(), FwErrors>;

    fn imdb_data(&self) -> Option<&imdb::IMDbTitle>;

    fn imdb_data_owned(&mut self) -> Option<imdb::IMDbTitle>;

    fn imdb_lookup(&mut self, imdb: &IMDb) -> Result<imdb::IMDbTitle, FwErrors> {
        let year = match &mut self.year() {
            Year::OneYear(year) | Year::Range(year, _) => *year,
        };

        while let Some((ref alternate_title, score)) = self.alter_titles().as_mut().unwrap().pop() {
            if score == 0 {
                break;
            }

            if let Ok(imdb_title) = imdb.advanced_search(&alternate_title.title, year, year) {
                return Ok(imdb_title);
            }

            if let Ok(imdb_title) =
                imdb.search(&format!("{} {}", &alternate_title.title, self.year()))
            {
                return Ok(imdb_title);
            }
        }
        Err(FwErrors::ZeroResults)
    }
}

pub trait RatedTitle: Title {
    fn rating(&self) -> Option<u8>;
    fn is_favorited(&self) -> bool;
    fn is_watchlisted(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_a_year() {
        let year1 = Year::from_str("(2015-2017)");
        let year2 = Year::from_str("1984-2021");
        let year3 = Year::from_str("2040");
        assert_eq!((year1.start(), year1.end()), (2015, 2017));
        assert_eq!((year2.start(), year2.end()), (1984, 2021));
        assert_eq!((year3.start(), year3.end()), (2040, 2040));
    }
}
