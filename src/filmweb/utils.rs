use super::FilmwebGenre;
use crate::{AlternateTitle, FilmwebErrors, Year};

use priority_queue::PriorityQueue;
use reqwest::blocking::Client;
use scraper::{ElementRef, Html, Selector};

use super::STR_TO_GENRE;

pub struct ScrapedFilmwebTitleData {
    pub id: u32,
    pub year: Year,
    pub name: String,
    pub url: String,
    pub genres: Vec<FilmwebGenre>,
    pub alter_titles: PriorityQueue<AlternateTitle, u8>,
    pub duration: Option<u16>, // in minutes
}

pub fn parse_my_votebox(
    votebox: ElementRef,
    client: &Client,
) -> Result<ScrapedFilmwebTitleData, FilmwebErrors> {
    let id = votebox
        .select(&Selector::parse(".previewFilm").expect("selector ok"))
        .next()
        .expect("filmweb hasnt changed")
        .value()
        .attr("data-film-id")
        .expect("attribute exists")
        .trim()
        .parse::<u32>()?;
    let year = {
        let year = votebox
            .select(&Selector::parse(".preview__year").expect("selector ok"))
            .next()
            .expect("filmweb api hasn't changed")
            .inner_html();
        if year.contains('-') {
            let years = year.trim().split('-').collect::<Vec<&str>>();
            let year_start = years[0].trim().parse::<u16>()?;
            let year_end = years[1]
                .trim()
                .parse::<u16>()
                .map_or(year_start, |year| year);
            Year::Range(year_start, year_end)
        } else {
            match year.trim().parse::<u16>() {
                Ok(year) => Year::OneYear(year),
                Err(_) => {
                    return Err(FilmwebErrors::InvalidYear {
                        title_id: id,
                        failed_year: year,
                    })
                }
            }
        }
    };

    let name = votebox
        .select(&Selector::parse(".preview__link").expect("selector ok"))
        .next()
        .expect("filmweb hasnt changed")
        .inner_html();

    let genres: Vec<FilmwebGenre> = votebox
        .select(&Selector::parse(".preview__detail--genres h3 a").expect("selector ok"))
        .into_iter()
        .inspect(|genre| {
            // TODO: improve this, make a target
            log::info!("Genre {}", &genre.inner_html());
        })
        .map(|genre| {
            *STR_TO_GENRE
                .get(genre.inner_html().trim().to_lowercase().as_str())
                .expect("filmweb didnt added new genres")
        })
        .collect();
    assert!(!genres.is_empty(), "There should be atleast one genre");

    let title_url: String = format!(
        "https://filmweb.pl{}",
        votebox
            .select(&Selector::parse(".preview__link").expect("selector ok"))
            .next()
            .expect("filmweb hasnt changed")
            .value()
            .attr("href")
            .expect("filmweb hasnt changed")
    );

    let alter_titles_url = format!("{title_url}/titles");
    let alter_titles = AlternateTitle::fw_get_titles(&alter_titles_url, client)?;

    let duration = {
        let document = {
            let res = client.get(&title_url).send()?.text()?;
            Html::parse_document(&res)
        };

        document
            .select(&Selector::parse(".filmCoverSection__duration").expect("selector ok"))
            .next()
            .expect("filmweb hasnt changed")
            .value()
            .attr("data-duration")
            .expect("filmweb hasnt changed")
            .parse::<u16>()
            .map_or_else(
                |_| {
                    log::info!("Duration not found for {title_url}");
                    None
                },
                Some,
            )
    };
    Ok(ScrapedFilmwebTitleData {
        id,
        year,
        genres,
        name,
        url: title_url,
        alter_titles,
        duration,
    })
}

#[cfg(test)]
mod tests {}
