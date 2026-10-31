use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use crate::AppState;

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

// ponytail: first Wikidata hit by title whose author shares a name word wins; a book in two series
// (Discworld and its Rincewind arc) gets whichever comes first. Add pq:P1545 here once series_index exists.
pub async fn lookup(
    http: &reqwest::Client,
    title: &str,
    author: &str,
    lang: Option<&str>,
) -> reqwest::Result<Option<String>> {
    let lang = crate::legal::lang_code(lang);
    let words = name_words(author);
    if lang.len() != 2 || words.is_empty() {
        return Ok(None);
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
        r#"SELECT ?seriesLabel WHERE {{
  SERVICE wikibase:mwapi {{
    bd:serviceParam wikibase:endpoint "www.wikidata.org"; wikibase:api "EntitySearch";
      mwapi:search {}; mwapi:language "{lang}"; mwapi:limit "5".
    ?item wikibase:apiOutputItem mwapi:item.
  }}
  ?item wdt:P179 ?series; wdt:P50 ?a.
  ?a rdfs:label|skos:altLabel ?an.
  FILTER({filter})
  SERVICE wikibase:label {{ bd:serviceParam wikibase:language "{lang},en". }}
}} LIMIT 1"#,
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
    let series = body["results"]["bindings"][0]["seriesLabel"]["value"]
        .as_str()
        .filter(|s| !s.starts_with('Q') || !s[1..].chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string);
    Ok(crate::parse::Meta {
        series,
        ..Default::default()
    }
    .sanitized()
    .series)
}

// ponytail: upload waits at most 5 s for Wikidata; a slow or failed answer leaves the series empty
// and the user fills it in the form.
pub async fn on_upload(
    http: &reqwest::Client,
    title: &str,
    author: &str,
    lang: Option<&str>,
) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(5), lookup(http, title, author, lang)).await {
        Ok(Ok(series)) => series,
        Ok(Err(e)) => {
            tracing::warn!("series lookup of {title}: {e}");
            None
        }
        Err(_) => None,
    }
}

pub async fn backfill(state: Arc<AppState>) {
    let books: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
        "select id, title, author, lang from books where series is null and series_checked_at is null",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    for (id, title, author, lang) in books {
        match lookup(&state.http, &title, &author, lang.as_deref()).await {
            Ok(series) => {
                if let Err(e) = sqlx::query(
                    "update books set series_checked_at = now(), series = coalesce(series, $2) where id = $1",
                )
                .bind(id)
                .bind(&series)
                .execute(&state.db)
                .await
                {
                    tracing::warn!("series lookup of {id}: {e}");
                }
            }
            Err(e) => tracing::warn!("series lookup of {id}: {e}"),
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
}
