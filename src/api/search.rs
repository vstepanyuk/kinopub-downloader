use cli_table::{format::Justify, Table};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Genre {
    // id: u64,
    title: String,
}

#[derive(Debug, Deserialize, Table)]
pub struct SearchResultItem {
    #[table(title = "ID", justify = "Justify::Right")]
    pub id: u64,
    #[table(title = "Title")]
    pub title: String,
    #[table(title = "Year")]
    pub year: u16,
    #[table(title = "Type")]
    pub r#type: String,
    #[serde(rename = "plot")]
    #[table(skip)]
    pub description: String,
    #[table(skip)]
    pub imdb_rating: Option<f32>,
    #[table(skip)]
    pub kinopoisk_rating: Option<f32>,

    #[table(title = "Genre", display_fn = "render_genres")]
    pub genres: Vec<Genre>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
}

fn render_genres(items: &[Genre]) -> String {
    items
        .iter()
        .map(|genre| genre.title.to_owned())
        .collect::<Vec<_>>()
        .join(", ")
}
