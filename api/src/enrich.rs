use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use crate::AppState;

const GENRES_MAX: usize = 6;
const GENRE_MAX: usize = 60;

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub series: Option<String>,
    pub series_index: Option<f32>,
    pub genres: Vec<String>,
    pub description: Option<String>,
}

fn name_words(author: &str) -> Vec<String> {
    crate::legal::authors(author)
        .first()
        .map(|a| {
            a.split_whitespace()
                .map(|w| w.trim_matches('.').to_lowercase())
                .filter(|w| w.chars().count() >= 3)
                .collect()
        })
        .unwrap_or_default()
}

fn is_qid(s: &str) -> bool {
    s.strip_prefix('Q')
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn genres(joined: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in joined.split('|') {
        let g = crate::parse::clean(g, GENRE_MAX).to_lowercase();
        if !g.is_empty() && !out.contains(&g) {
            out.push(g);
        }
    }
    out.truncate(GENRES_MAX);
    out
}

// ponytail: first Wikidata work by title whose author shares a name word wins; a book in two series
// (Discworld and its Rincewind arc) gets whichever comes first. Genres are labels in the book's
// language, so a library in several languages shows "фэнтези" and "fantasy" side by side.
pub async fn lookup(
    http: &reqwest::Client,
    title: &str,
    author: &str,
    lang: Option<&str>,
    want_description: bool,
) -> reqwest::Result<Found> {
    let lang = crate::legal::lang_code(lang);
    let words = name_words(author);
    if lang.len() != 2 || words.is_empty() {
        return Ok(Found::default());
    }
    let title: String = title.chars().take(100).collect();
    let filter = words
        .iter()
        .map(|w| {
            format!(
                "CONTAINS(LCASE(?an), {})",
                serde_json::Value::String(w.clone())
            )
        })
        .collect::<Vec<_>>()
        .join(" || ");
    let query = format!(
        r#"SELECT ?seriesLabel ?ord ?article (GROUP_CONCAT(DISTINCT ?genreName; separator="|") AS ?genres) WHERE {{
  SERVICE wikibase:mwapi {{
    bd:serviceParam wikibase:endpoint "www.wikidata.org"; wikibase:api "EntitySearch";
      mwapi:search {}; mwapi:language "{lang}"; mwapi:limit "5".
    ?item wikibase:apiOutputItem mwapi:item.
  }}
  ?item wdt:P50 ?a.
  ?a rdfs:label|skos:altLabel ?an.
  FILTER({filter})
  OPTIONAL {{ ?item p:P179 ?st. ?st ps:P179 ?series. OPTIONAL {{ ?st pq:P1545 ?ord }} }}
  OPTIONAL {{ ?item wdt:P136 ?genre. ?genre rdfs:label ?genreName. FILTER(LANG(?genreName) = "{lang}") }}
  OPTIONAL {{ ?article schema:about ?item; schema:isPartOf <https://{lang}.wikipedia.org/>. }}
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "{lang},en". }}
}} GROUP BY ?item ?seriesLabel ?ord ?article LIMIT 1"#,
        serde_json::Value::String(title)
    );
    let body: serde_json::Value = http
        .get("https://query.wikidata.org/sparql")
        .query(&[("format", "json"), ("query", &query)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let row = &body["results"]["bindings"][0];
    let field = |key: &str| row[key]["value"].as_str().filter(|v| !v.is_empty());

    let series = field("seriesLabel")
        .filter(|s| !is_qid(s))
        .map(str::to_string);
    let mut found = Found {
        series_index: series
            .as_ref()
            .and(field("ord"))
            .and_then(|o| o.parse().ok()),
        series,
        genres: field("genres").map(genres).unwrap_or_default(),
        description: None,
    };
    if let Some(page) = field("article")
        .and_then(|a| a.rsplit('/').next())
        .filter(|_| want_description)
    {
        let summary: serde_json::Value = http
            .get(format!(
                "https://{lang}.wikipedia.org/api/rest_v1/page/summary/{page}"
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        found.description = summary["extract"].as_str().map(str::to_string);
    }
    let meta = crate::parse::Meta {
        series: found.series,
        description: found.description,
        ..Default::default()
    }
    .sanitized();
    found.series = meta.series;
    found.description = meta.description;
    Ok(found)
}

// ponytail: upload waits at most 5 s for Wikidata and Wikipedia; a slow or failed answer leaves
// enriched_at empty and the startup backfill tries again.
pub async fn on_upload(
    http: &reqwest::Client,
    title: &str,
    author: &str,
    lang: Option<&str>,
    want_description: bool,
) -> Option<Found> {
    match tokio::time::timeout(
        Duration::from_secs(5),
        lookup(http, title, author, lang, want_description),
    )
    .await
    {
        Ok(Ok(found)) => Some(found),
        Ok(Err(e)) => {
            tracing::warn!("enrich lookup of {title}: {e}");
            None
        }
        Err(_) => None,
    }
}

pub async fn apply<'e>(db: impl sqlx::PgExecutor<'e>, id: Uuid, found: &Found) -> sqlx::Result<()> {
    sqlx::query(
        "update books set enriched_at = now(),
             series = coalesce(series, $2),
             series_index = coalesce(series_index,
                 case when coalesce(series, $2) = $2 then $3 end),
             description = coalesce(description, $4),
             genres = case when genres = '{}' then $5 else genres end
         where id = $1",
    )
    .bind(id)
    .bind(&found.series)
    .bind(found.series_index)
    .bind(&found.description)
    .bind(&found.genres)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn backfill(state: Arc<AppState>) {
    let books: Vec<(Uuid, String, String, Option<String>, bool)> = sqlx::query_as(
        "select id, title, author, lang, description is null from books where enriched_at is null",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for (id, title, author, lang, want_description) in books {
        match lookup(
            &state.http,
            &title,
            &author,
            lang.as_deref(),
            want_description,
        )
        .await
        {
            Ok(found) => {
                if let Err(e) = apply(&state.db, id, &found).await {
                    tracing::warn!("enrich lookup of {id}: {e}");
                }
            }
            Err(e) => tracing::warn!("enrich lookup of {id}: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_name_words_come_from_the_first_author() {
        assert_eq!(name_words("Дж. К. Роулинг, Кто-то Ещё"), ["роулинг"]);
        assert_eq!(name_words("Лев Толстой"), ["лев", "толстой"]);
        assert!(name_words("").is_empty());
    }

    #[test]
    fn genres_are_deduplicated_and_capped() {
        assert_eq!(
            genres("Фэнтези|приключения|фэнтези|"),
            ["фэнтези", "приключения"]
        );
        assert_eq!(genres("a|b|c|d|e|f|g|h").len(), GENRES_MAX);
        assert!(is_qid("Q123") && !is_qid("Q") && !is_qid("Quest"));
    }

    #[tokio::test]
    #[ignore = "hits live Wikidata and Wikipedia"]
    async fn finds_harry_potter() {
        let found = lookup(
            &crate::sources::client().unwrap(),
            "Гарри Поттер и Тайная комната",
            "Джоан Роулинг",
            Some("ru"),
            true,
        )
        .await
        .unwrap();
        assert_eq!(found.series.as_deref(), Some("Гарри Поттер"));
        assert_eq!(found.series_index, Some(2.0));
        assert!(found.genres.contains(&"фэнтези".to_string()));
        assert!(found.description.is_some_and(|d| d.contains("Хогвартс")));
    }
}
