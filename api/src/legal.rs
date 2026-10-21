use std::{sync::Arc, time::Duration};

use sqlx::types::chrono::Utc;
use uuid::Uuid;

use crate::AppState;

/// Years after the author's death until the work is free in Poland, Ukraine and the rest of the EU.
const TERM: i32 = 70;

pub struct Person {
    id: String,
    died: Option<i32>,
    langs: Vec<String>,
}

fn authors(field: &str) -> Vec<String> {
    let mut field = field.to_string();
    for joint in [" и ", " і ", " i ", " and ", " und ", "&", ";"] {
        field = field.replace(joint, ",");
    }
    field
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_string)
        .collect()
}

/// Free only when every person who answers to every listed name has been dead for over 70 full years,
/// and the book is in a language one of them wrote in: a translation carries the translator's own
/// copyright, which the metadata does not name.
// ponytail: an unknown author, a namesake who is alive or a missing language all mean "not free";
// the admin can still open such a book by hand.
pub fn verdict(found: &[Vec<Person>], lang: &str, year: i32) -> Option<String> {
    if found.is_empty() || found.iter().any(Vec::is_empty) {
        return None;
    }
    let people = || found.iter().flatten();
    let free = people().all(|p| p.died.is_some_and(|d| d + TERM < year));
    let original = people().any(|p| p.langs.iter().any(|l| l == lang));
    (free && original).then(|| {
        people()
            .map(|p| format!("{} † {}", p.id, p.died.unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(", ")
    })
}

async fn people(http: &reqwest::Client, name: &str, lang: &str) -> reqwest::Result<Vec<Person>> {
    let name: String = name.chars().take(100).collect();
    let query = format!(
        r#"SELECT ?item (MAX(YEAR(?death)) AS ?died) (GROUP_CONCAT(DISTINCT ?code) AS ?langs) WHERE {{
  SERVICE wikibase:mwapi {{
    bd:serviceParam wikibase:endpoint "www.wikidata.org"; wikibase:api "EntitySearch";
      mwapi:search {}; mwapi:language "{lang}"; mwapi:limit "5".
    ?item wikibase:apiOutputItem mwapi:item.
  }}
  ?item wdt:P31 wd:Q5.
  OPTIONAL {{ ?item wdt:P570 ?death }}
  OPTIONAL {{ ?item wdt:P1412/wdt:P218 ?code }}
}} GROUP BY ?item"#,
        serde_json::Value::String(name)
    );
    let body: serde_json::Value = http
        .get("https://query.wikidata.org/sparql")
        .query(&[("format", "json"), ("query", &query)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let field = |row: &serde_json::Value, key: &str| row[key]["value"].as_str().map(str::to_string);
    Ok(body["results"]["bindings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| Person {
            id: field(row, "item")
                .and_then(|u| u.rsplit('/').next().map(str::to_string))
                .unwrap_or_default(),
            died: field(row, "died").and_then(|y| y.parse().ok()),
            langs: field(row, "langs")
                .map(|l| l.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default(),
        })
        .collect())
}

/// Looks the book's authors up in Wikidata and opens the book to everyone when it is in the public domain.
/// It never closes a book: what the admin opened by hand stays open.
pub async fn check(state: Arc<AppState>, id: Uuid) {
    let row: Option<(String, Option<String>)> =
        match sqlx::query_as("select author, lang from books where id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await
        {
            Ok(row) => row,
            Err(e) => return tracing::warn!("legal check of {id}: {e}"),
        };
    let Some((author, lang)) = row else { return };
    let lang: String = lang
        .unwrap_or_default()
        .to_lowercase()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();

    let mut found = Vec::new();
    if lang.len() == 2 {
        for name in authors(&author).iter().take(4) {
            match people(&state.http, name, &lang).await {
                Ok(p) => found.push(p),
                Err(e) => return tracing::warn!("legal check of {id}: {e}"),
            }
        }
    }
    let year = Utc::now().format("%Y").to_string().parse().unwrap_or(0);
    let basis = verdict(&found, &lang, year);
    if let Err(e) = sqlx::query(
        "update books set legal_checked_at = now(),
             public = public or $2 is not null, public_basis = coalesce($2, public_basis)
         where id = $1",
    )
    .bind(id)
    .bind(&basis)
    .execute(&state.db)
    .await
    {
        tracing::warn!("legal check of {id}: {e}");
    }
}

/// Books uploaded before the check existed, or whose check hit a network error, are looked up once
/// at startup, slowly, to stay polite to Wikidata.
pub async fn backfill(state: Arc<AppState>) {
    let ids: Vec<(Uuid,)> = sqlx::query_as("select id from books where legal_checked_at is null")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    for (id,) in ids {
        check(state.clone(), id).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(died: Option<i32>, langs: &[&str]) -> Person {
        Person {
            id: "Q1".into(),
            died,
            langs: langs.iter().map(|l| l.to_string()).collect(),
        }
    }

    #[test]
    fn only_long_dead_authors_in_their_own_language_are_free() {
        let tolstoy = || vec![person(Some(1910), &["ru"]), person(Some(1945), &["ru"])];
        assert!(verdict(&[tolstoy()], "ru", 2026).is_some());
        assert!(verdict(&[tolstoy()], "en", 2026).is_none(), "a translation");
        assert!(
            verdict(&[vec![person(Some(1956), &["ru"])]], "ru", 2026).is_none(),
            "70 years not over"
        );
        assert!(verdict(&[vec![person(Some(1955), &["ru"])]], "ru", 2026).is_some());
        assert!(
            verdict(&[vec![person(None, &["pl"])]], "pl", 2026).is_none(),
            "alive"
        );
        assert!(
            verdict(&[tolstoy(), vec![]], "ru", 2026).is_none(),
            "an unknown co-author"
        );
        assert!(verdict(&[], "ru", 2026).is_none(), "no author");
        let namesake = vec![person(Some(1881), &["ru"]), person(None, &[])];
        assert!(
            verdict(&[namesake], "ru", 2026).is_none(),
            "a living namesake"
        );
    }

    #[test]
    fn author_lists_split_into_people() {
        assert_eq!(
            authors("Анджей Сапковский, Денис Гордеев"),
            ["Анджей Сапковский", "Денис Гордеев"]
        );
        assert_eq!(authors("Ильф и Петров"), ["Ильф", "Петров"]);
        assert!(authors("  ").is_empty());
    }
}
