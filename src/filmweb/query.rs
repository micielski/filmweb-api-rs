use super::FilmwebGenre;
use crate::Year;

#[derive(Debug, Clone)]
pub struct QueryBuilder {
    year: Option<Year>,
    genres: Option<Vec<FilmwebGenre>>,
}

impl QueryBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            year: None,
            genres: None,
        }
    }

    #[must_use]
    pub const fn year(mut self, year: Year) -> Self {
        self.year = Some(year);
        self
    }

    #[must_use]
    pub fn genres(mut self, genres: Vec<FilmwebGenre>) -> Self {
        self.genres = Some(genres);
        self
    }

    #[must_use]
    pub fn build(self) -> Query {
        let year_param = match self.year {
            None => "startYear=1890&endYear=2060".to_string(),
            Some(Year::OneYear(year)) => format!("startYear={year}&endYear={year}"),
            Some(Year::Range(start, end)) => format!("startYear={start}&endYear={end}"),
        };

        let genres_param = {
            if self.genres.is_none() {
                String::new()
            } else {
                let mut genres_param = String::new();
                for genre in self.genres.expect("Should be atleast one genre") {
                    genres_param.push_str(&format!("{},", genre as u8));
                }
                let len = genres_param.len();
                format!("&genres={}", &genres_param[..len - 1])
            }
        };

        let url = format!(
            "https://www.filmweb.pl/api/v1/films/search?{year_param}{genres_param}&connective=OR"
        );
        dbg!(&url);
        Query(url)
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Query(String);

impl Query {
    #[must_use]
    pub fn url(&self, page: u16) -> String {
        format!("{}&page={page}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creating_query_with_year() {
        let year = Year::new(2010, 2017);
        let query = QueryBuilder::new().year(year).build();

        assert_eq!("https://www.filmweb.pl/api/v1/films/search?startYear=2010&endYear=2017&connective=OR&page=10", query.url(10));
    }

    #[test]
    fn creating_query_with_year_and_genres() {
        let query = QueryBuilder::new()
            .year(Year::new(2010, 2017))
            .genres(vec![FilmwebGenre::Thriller, FilmwebGenre::Psychological])
            .build();

        assert_eq!("https://www.filmweb.pl/api/v1/films/search?startYear=2010&endYear=2017&genres=24,38&connective=OR&page=10", query.url(10));
    }

    #[test]
    fn creating_query_with_year_and_genres2() {
        let query = QueryBuilder::new()
            .year(Year::new(2021, 2021))
            .genres(vec![
                FilmwebGenre::Comedy,
                FilmwebGenre::Drama,
                FilmwebGenre::SciFi,
            ])
            .build();
        assert_eq!("https://www.filmweb.pl/api/v1/films/search?startYear=2021&endYear=2021&genres=13,6,33&connective=OR&page=1", query.url(1));
    }
}
