use crate::{error::IMDbScrapeError, utils::create_client, Genre, Title, TitleID, TitleType, Year};
use std::str::FromStr;

use once_cell::sync::OnceCell;
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct IMDbTitle {
    title: String,
    #[serde(skip)]
    url: OnceCell<String>,
    pub id: TitleID,
    year: Year,
    duration: u16,
    genres: Vec<Genre>,
    title_type: TitleType,
}

impl Title for IMDbTitle {
    fn url(&self) -> &String {
        if self.url.get().is_none() {}
        todo!()
    }

    fn duration(&self) -> Option<u16> {
        Some(self.duration)
    }

    fn title_type(&self) -> &TitleType {
        &self.title_type
    }

    fn id(&self) -> &TitleID {
        &self.id
    }

    fn title(&self) -> &String {
        &self.title
    }

    fn genres(&self) -> &Vec<Genre> {
        &self.genres
    }

    fn year(&self) -> Year {
        self.year
    }
}

pub struct IMDb(Client);

impl Default for IMDb {
    fn default() -> Self {
        Self::new()
    }
}

impl IMDb {
    /// Returns a queryable `IMDb` struct
    ///
    /// # Examples
    /// ```rust
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// use filmed::Title;
    /// use filmed::imdb::IMDb;
    /// let imdb = IMDb::new();
    /// let stay = imdb.search("Stay 2005")?;
    /// assert_eq!(stay.title(), "Zostań"); // the title should be in english for you
    /// #     Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(create_client().expect("can create a client"))
    }

    fn parse_imdb_title_page(&self, id: &str) -> Result<ScrapedIMDbTitlePageData, IMDbScrapeError> {
        let title_url = format!("https://www.imdb.com/title/{id}/");
        let response = self.0.get(&title_url).send()?.text()?;
        let dom = tl::parse(&response, tl::ParserOptions::default()).unwrap();
        let parser = dom.parser();
        let genres: Vec<Genre> = {
            dom.query_selector(".ipc-chip__text")
                .unwrap()
                .filter_map(|handle| {
                    let node = handle.get(parser).unwrap();
                    Genre::try_from(&*node.inner_text(parser)).ok()
                })
                .collect()
        };

        if genres.is_empty() {
            return Err(IMDbScrapeError::GenreParseError {
                bad_title_url: title_url,
            });
        }

        let get_dirty_duration = |nth| {
            let handle = dom
                .query_selector(".ipc-inline-list__item")
                .unwrap()
                .nth(nth)
                .expect("Panic occured while trying to export {title} {year}");
            handle.get(parser).unwrap().inner_text(parser)
        };

        let mut x = 4;
        let dirty_duration = {
            loop {
                let duration_candidate = get_dirty_duration(x);
                if Self::is_dirty_duration_ok(&duration_candidate) {
                    break duration_candidate;
                };
                if x == 7 {
                    return Err(IMDbScrapeError::IrrecoverableParseDurationError {
                        bad_string: duration_candidate.to_string(),
                        title_url: title_url.to_string(),
                    });
                }
                x += 1;
            }
        };

        let duration = Self::parse_dirty_duration(&dirty_duration, &title_url)?;

        let title_type = {
            let page_title = {
                dom.query_selector("title")
                    .unwrap()
                    .next()
                    .unwrap()
                    .get(parser)
                    .unwrap()
                    .inner_text(parser)
            };
            if page_title.contains("TV") && page_title.contains("Series") {
                TitleType::Show
            } else {
                TitleType::Movie
            }
        };

        Ok(ScrapedIMDbTitlePageData {
            genres,
            duration,
            title_type,
        })
    }

    fn is_dirty_duration_ok(dirty_duration: &str) -> bool {
        if dirty_duration
            .chars()
            .all(|c| matches!(c, 'h' | 'm' | ' ' | '0'..='9'))
            && dirty_duration.len() < 20
            && !dirty_duration.chars().all(|c| char::is_ascii_digit(&c))
        {
            true
        } else {
            log::info!("Bad IMDb dirty duration: {dirty_duration}");
            false
        }
    }

    // Example of dirty_duration: "1h 33m"
    fn parse_dirty_duration(dirty_duration: &str, title_url: &str) -> Result<u16, IMDbScrapeError> {
        let dirty_duration: Vec<u16> = {
            let duration = dirty_duration.split_once(' ');
            match duration {
                Some((hours, mins)) if !mins.is_empty() => {
                    let h_len = hours.len();
                    let m_len = mins.len();
                    let hours = match hours[..h_len - 1].parse::<u16>() {
                        Ok(hours) => hours,
                        Err(_) => {
                            return Err(IMDbScrapeError::IrrecoverableParseDurationError {
                                bad_string: dirty_duration.to_string(),
                                title_url: title_url.to_string(),
                            })
                        }
                    };

                    let mins = match mins[..m_len - 1].parse::<u16>() {
                        Ok(mins) => mins,
                        Err(_) => {
                            return Err(IMDbScrapeError::IrrecoverableParseDurationError {
                                bad_string: dirty_duration.to_string(),
                                title_url: title_url.to_string(),
                            })
                        }
                    };
                    vec![hours, mins]
                }
                Some(_) => {
                    unreachable!();
                }
                None => {
                    let len = dirty_duration.len();
                    match dirty_duration.chars().nth(len - 1) {
                        Some('h') => {
                            vec![dirty_duration[..len - 1].parse::<u16>().expect("IMDb ok") * 60]
                        }
                        Some('m') => {
                            vec![dirty_duration[..len - 1].parse::<u16>().expect("IMDb ok") * 60]
                        }
                        _ => {
                            return Err(IMDbScrapeError::IrrecoverableParseDurationError {
                                bad_string: dirty_duration.to_string(),
                                title_url: title_url.to_string(),
                            })
                        }
                    }
                }
            }
        };
        if dirty_duration.len() >= 2 {
            Ok(dirty_duration[0] * 60 + dirty_duration[1])
        } else {
            Ok(dirty_duration[0])
        }
    }

    pub fn advanced_search(
        &self,
        title: &str,
        year_start: u16,
        year_end: u16,
    ) -> Result<IMDbTitle, IMDbScrapeError> {
        let search_page_url = format!(
            "https://www.imdb.com/search/title/?title={}&release_date={},{}&adult=include",
            title, year_start, year_end
        );

        let search_document = {
            let response = self.0.get(&search_page_url).send()?.text()?;
            Html::parse_document(&response)
        };

        let title_data = if let Some(id) = search_document
            .select(&Selector::parse("div.lister-item-image").expect("selector ok"))
            .next()
        {
            id
        } else {
            log::info!(
            "Failed to get a match in Fn get_imdb_data_advanced for {title} {year_start} on {search_page_url}"
        );
            return Err(IMDbScrapeError::NoResults {
                search_url: format!("{} {}", title.to_owned(), year_start),
            });
        };

        let id = {
            let id = title_data.inner_html();
            let regex = Regex::new(r"(\d{7,8})").expect("regex ok");
            format!("tt{:0>7}", &regex.captures(&id).unwrap()[0])
                .trim()
                .to_string()
        };
        log::debug!("Found a potential IMDb id for {title} {year_start} on {search_page_url}");

        let title = search_document
            .select(&Selector::parse("img.loadlate").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("alt")
            .unwrap();

        // .inner_html gives (2015-2016) / (2015)
        let year: Year = {
            let dirty_year = search_document
                .select(&Selector::parse(".lister-item-year").unwrap())
                .next()
                .unwrap()
                .inner_html();
            match Year::from_str(&dirty_year) {
                Ok(year) => Ok(year),
                Err(e) => Err(IMDbScrapeError::IrrecoverableParseYearError {
                    title_url: search_page_url,
                    source: e,
                }),
            }?
        };

        let ScrapedIMDbTitlePageData {
            genres,
            duration,
            title_type,
        } = self.parse_imdb_title_page(&id)?;

        let imdb_data = IMDbTitle {
            id: TitleID::IMDbID(id),
            url: OnceCell::new(),
            year,
            genres,
            title: title.to_string(),
            duration,
            title_type,
        };

        Ok(imdb_data)
    }

    pub fn search(&self, title: &str) -> Result<IMDbTitle, IMDbScrapeError> {
        let url_query = format!("https://www.imdb.com/find?q={title}");
        let document = {
            let response = self.0.get(&url_query).send()?.text()?;
            Html::parse_document(&response)
        };

        let title = if let Some(title) = document
            .select(&Selector::parse(".ipc-metadata-list-summary-item__t").expect("selector ok"))
            .next()
        {
            title.inner_html()
        } else {
            return Err(IMDbScrapeError::NoResults {
                search_url: url_query,
            });
        };

        let year = match Year::from_str(
            &document
                .select(
                    &Selector::parse(".ipc-metadata-list-summary-item__li").expect("selector ok"),
                )
                .next()
                .expect("selector is ok")
                .inner_html(),
        ) {
            Ok(year) => year,
            Err(e) => {
                return Err(IMDbScrapeError::IrrecoverableParseYearError {
                    title_url: url_query,
                    source: e,
                })
            }
        };

        // Should give something like: /title/tt4158110/?ref_=fn_al_tt_1
        let dirty_id = document
            .select(&Selector::parse(".ipc-metadata-list-summary-item__t").expect(""))
            .next()
            .unwrap()
            .value()
            .attr("href")
            .unwrap(); // TODO: return Outdated err
        let regex_id = Regex::new(r"(\d{7,8})").unwrap();
        let id = format!(
            "tt{:0>7}",
            regex_id
                .captures(dirty_id)
                .unwrap()
                .get(0)
                .unwrap()
                .as_str()
        );

        let ScrapedIMDbTitlePageData {
            genres,
            duration,
            title_type,
        } = self.parse_imdb_title_page(&id)?;

        let imdb_data = IMDbTitle {
            id: TitleID::IMDbID(id),
            url: OnceCell::new(),
            year,
            genres,
            title_type,
            title,
            duration,
        };

        Ok(imdb_data)
    }
}

struct ScrapedIMDbTitlePageData {
    genres: Vec<Genre>,
    duration: u16,
    title_type: TitleType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searching_imdb() {
        let imdb = IMDb::new();
        let stay = imdb.search("Stay 2005").unwrap();
        assert_eq!(stay.title(), "Zostań"); // why the hell this is in polish for me TODO ?
        assert_eq!(stay.year().start(), 2005);
        assert_eq!(stay.year().end(), 2005);
        assert_eq!(stay.duration(), Some(99));
        assert_eq!(
            *stay.genres(),
            vec![Genre::Drama, Genre::Mystery, Genre::Thriller]
        )
    }

    #[test]
    fn searching_imdb2() {
        let imdb = IMDb::new();
        let movie = imdb.search("Josee to Tora to Sakana-tachi").unwrap();
        assert_eq!(movie.year().start(), 2020);
    }

    #[test]
    fn searching_imdb3() {
        let imdb = IMDb::new();
        let the_whale = imdb.search("The Whale 2022").unwrap();
        assert_eq!(the_whale.year().start(), 2022);
    }

    #[test]
    fn parsing_dirty_duration() {
        let first = IMDb::parse_dirty_duration("2h", "2h test").unwrap();
        let second = IMDb::parse_dirty_duration("2h 12m", "2h 12m test").unwrap();
        assert_eq!(first, 120);
        assert_eq!(second, 132);
    }
}
