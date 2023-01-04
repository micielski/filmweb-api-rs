use super::{Client, Deserialize, FwErrors, Html, Selector, Serialize};
use regex::Regex;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImdbTitle {
    pub title: String,
    pub id: String,
    pub duration: u32,
}

pub fn advanced_imdb_search(
    title: &str,
    year_start: u16,
    year_end: u16,
    imdb_client: &Client,
) -> Result<ImdbTitle, Box<dyn std::error::Error>> {
    let url = format!(
        "https://www.imdb.com/search/title/?title={}&release_date={},{}&adult=include",
        title, year_start, year_end
    );

    let document = {
        let response = imdb_client.get(&url).send()?.text()?;
        Html::parse_document(&response)
    };

    let title_data = if let Some(id) = document
        .select(&Selector::parse("div.lister-item-image").unwrap())
        .next()
    {
        id
    } else {
        log::info!(
            "Failed to get a match in Fn get_imdb_data_advanced for {title} {year_start} on {url}"
        );
        return Err(Box::new(FwErrors::ZeroResults));
    };

    let title_id = {
        let id = title_data.inner_html();
        let regex = Regex::new(r"(\d{7,8})").unwrap();
        format!("tt{:0>7}", &regex.captures(&id).unwrap()[0])
            .trim()
            .to_string()
    };
    log::debug!("Found a potential IMDb id for {title} {year_start} on {url}");

    let imdb_title = document
        .select(&Selector::parse("img.loadlate").unwrap())
        .next()
        .unwrap()
        .value()
        .attr("alt")
        .unwrap();

    let duration = {
        let x = if let Some(a) = document
            .select(&Selector::parse(".runtime").unwrap())
            .next()
        {
            a.inner_html().replace(" min", "")
        } else {
            log::info!("Failed to fetch duration for {title} {year_start} on {url}");
            return Err(Box::new(FwErrors::InvalidDuration));
        };

        if let Ok(x) = x.parse::<u32>() {
            x
        } else {
            log::info!("Failed parsing duration to int for {title} {year_start} on {url}");
            return Err(Box::new(FwErrors::InvalidDuration));
        }
    };

    let imdb_data = ImdbTitle {
        id: title_id,
        title: imdb_title.to_string(),
        duration,
    };

    Ok(imdb_data)
}

pub fn imdb_search(
    title: &str,
    year: u16,
    imdb_client: &Client,
) -> Result<ImdbTitle, Box<dyn std::error::Error>> {
    let url_query = format!("https://www.imdb.com/find?q={}+{}", title, year);
    let document = {
        let response = imdb_client.get(&url_query).send()?.text()?;
        Html::parse_document(&response)
    };

    let imdb_title = if let Some(title) = document
        .select(&Selector::parse(".result_text a").unwrap())
        .next()
    {
        title.inner_html()
    } else {
        log::info!("No results in Fn get_imdb_data for {title} {year} on {url_query}");
        return Err(Box::new(FwErrors::ZeroResults));
    };

    let title_id = if let Some(id) = document
        .select(&Selector::parse(".result_text").unwrap())
        .next()
    {
        let title_id = id.inner_html();
        let re = Regex::new(r"(\d{7,8})").unwrap();
        format!(
            "tt{:0>7}",
            re.captures(title_id.as_str())
                .unwrap()
                .get(0)
                .unwrap()
                .as_str()
        )
    } else {
        log::info!("No results in Fn get_imdb_data for {title} {year} on {url_query}");
        return Err(Box::new(FwErrors::ZeroResults));
    };

    // get url of a title, and grab the duration
    let url = {
        let url_suffix = document
            .select(&Selector::parse("td.result_text a").unwrap())
            .next()
            .unwrap()
            .value()
            .attr("href")
            .unwrap();
        format!("https://www.imdb.com{}", url_suffix)
    };

    let document = {
        let response = imdb_client.get(&url).send()?.text()?;
        Html::parse_document(&response)
    };

    let get_dirty_duration = |nth| {
        document
            .select(&Selector::parse(".ipc-inline-list__item").unwrap())
            .nth(nth)
            .expect("Panic occured while trying to export {title} {year}")
            .inner_html()
    };

    let mut dirty_duration = get_dirty_duration(5);
    if dirty_duration.contains("Unrated")
        || dirty_duration.contains("Not Rated")
        || dirty_duration.contains("TV")
    {
        dirty_duration = get_dirty_duration(6);
    }

    if dirty_duration.len() > 40 {
        log::info!(
            "Invalid duration in Fn get_imdb_data on {url} for {title} {year} source: {url_query}"
        );
        return Err(Box::new(FwErrors::InvalidDuration));
    }

    // Example of dirty_duration: 1<!-- -->h<!-- --> <!-- -->33<!-- -->m<
    let duration = {
        let dirty_duration: Vec<u32> = dirty_duration
            .replace("<!-- -->", " ")
            .split_whitespace()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect();
        if dirty_duration.len() >= 2 {
            dirty_duration[0] * 60 + dirty_duration[1]
        } else {
            dirty_duration[0]
        }
    };
    log::debug!("Found duration {duration}m for {title} {year}");

    let imdb_data = ImdbTitle {
        id: title_id,
        title: imdb_title,
        duration,
    };

    Ok(imdb_data)
}
