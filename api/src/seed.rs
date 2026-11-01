use std::{sync::Arc, time::Duration};

use crate::{books::Stored, AppState, Result};

const GUTENBERG: [(u64, i32); 10] = [
    (1342, 1817),
    (84, 1851),
    (2701, 1891),
    (11, 1898),
    (1661, 1930),
    (345, 1912),
    (1513, 1616),
    (43, 1894),
    (174, 1900),
    (98, 1870),
];

const TEXTGRID: [&str; 8] = [
    "textgrid:11g9q.0",
    "textgrid:n18v.0",
    "textgrid:11cvr.0",
    "textgrid:v0fv.0",
    "textgrid:kscv.0",
    "textgrid:vtb3.0",
    "textgrid:106p3.0",
    "textgrid:mrds.0",
];

const WIKISOURCE: [(&str, &str, &str, i32); 12] = [
    (
        "ru",
        "Капитанская дочка (Пушкин)",
        "Капитанская дочка",
        1837,
    ),
    (
        "ru",
        "Герой нашего времени (Лермонтов)",
        "Герой нашего времени",
        1841,
    ),
    ("ru", "Мёртвые души (Гоголь)", "Мёртвые души", 1852),
    ("ru", "Горе от ума (Грибоедов)", "Горе от ума", 1829),
    ("ru", "Ревизор (Гоголь)", "Ревизор", 1852),
    ("ru", "Отцы и дети (Тургенев)", "Отцы и дети", 1883),
    ("uk", "Кайдашева сімя̀", "Кайдашева сім'я", 1918),
    ("uk", "Лісова пісня", "Лісова пісня", 1913),
    ("uk", "Тіні забутих предків", "Тіні забутих предків", 1913),
    ("uk", "Наталка Полтавка", "Наталка Полтавка", 1838),
    ("uk", "Захар Беркут", "Захар Беркут", 1916),
    ("uk", "Чорна рада (1943)", "Чорна рада", 1897),
];

const POLISH: usize = 8;
const AUDIO: usize = 3;
const AUDIO_MAX: u64 = 150 * 1024 * 1024;
const PATIENCE: Duration = Duration::from_secs(900);

#[derive(Debug, PartialEq)]
pub enum Pick {
    Gutenberg(u64, i32),
    TextGrid(&'static str),
    Wikisource {
        lang: &'static str,
        page: &'static str,
        title: &'static str,
        died: i32,
    },
    Wolne {
        slug: String,
        epub: String,
        page: String,
    },
    Audio(String),
}

impl Pick {
    pub fn key(&self) -> String {
        match self {
            Pick::Gutenberg(id, _) => format!("gutenberg:{id}"),
            Pick::TextGrid(uri) => (*uri).to_string(),
            Pick::Wikisource { lang, page, .. } => format!("wikisource:{lang}:{page}"),
            Pick::Wolne { slug, .. } => format!("wolnelektury:{slug}"),
            Pick::Audio(id) => format!("audio:{id}"),
        }
    }
}

pub fn fixed() -> Vec<Pick> {
    let en = GUTENBERG
        .iter()
        .map(|(id, died)| Pick::Gutenberg(*id, *died))
        .collect();
    let de = TEXTGRID.iter().map(|uri| Pick::TextGrid(uri)).collect();
    let (ru, uk) = WIKISOURCE
        .iter()
        .map(|(lang, page, title, died)| Pick::Wikisource {
            lang,
            page,
            title,
            died: *died,
        })
        .partition(|p| matches!(p, Pick::Wikisource { lang: "ru", .. }));
    crate::sources::interleave(vec![en, ru, de, uk])
}

async fn tried(state: &AppState, prefix: &str) -> i64 {
    sqlx::query_as::<_, (i64,)>("select count(*) from seeded where key like $1 || '%'")
        .bind(prefix)
        .fetch_one(&state.db)
        .await
        .map(|(n,)| n)
        .unwrap_or(i64::MAX)
}

async fn picks(state: &AppState) -> Vec<Pick> {
    let mut wolne = Vec::new();
    if tried(state, "wolnelektury:").await < POLISH as i64 {
        match crate::open::wolnelektury_top(&state.http, POLISH).await {
            Ok(found) => {
                wolne = found
                    .into_iter()
                    .filter_map(|f| {
                        let slug = f
                            .download_url
                            .rsplit('/')
                            .next()?
                            .strip_suffix(".epub")?
                            .to_string();
                        Some(Pick::Wolne {
                            slug,
                            epub: f.download_url,
                            page: f.page_url,
                        })
                    })
                    .collect()
            }
            Err(e) => tracing::warn!("seed: wolnelektury top: {e}"),
        }
    }
    let mut audio = Vec::new();
    if tried(state, "audio:").await < AUDIO as i64 {
        match crate::open::wolnelektury_audio_top(&state.http, AUDIO, AUDIO_MAX).await {
            Ok(ids) => audio = ids.into_iter().map(Pick::Audio).collect(),
            Err(e) => tracing::warn!("seed: wolnelektury audio top: {e}"),
        }
    }
    crate::sources::interleave(vec![fixed(), wolne, audio])
}

// ponytail: the library of plain readers is filled once from hand-picked and most-popular open books;
// every finished pick is remembered in `seeded`, so a book the admin deletes never comes back.
// Hand-picked books carry their author's death year instead of a Wikidata lookup, which trips over
// namesakes and pre-reform spelling; the list is short enough to check by eye.
pub async fn backfill(state: Arc<AppState>) {
    for pick in picks(&state).await {
        let key = pick.key();
        let done: Option<(String,)> = sqlx::query_as("select key from seeded where key = $1")
            .bind(&key)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
        if done.is_some() {
            continue;
        }
        match one(&state, &pick).await {
            Ok(()) => {
                if let Err(e) =
                    sqlx::query("insert into seeded (key) values ($1) on conflict do nothing")
                        .bind(&key)
                        .execute(&state.db)
                        .await
                {
                    tracing::warn!("seed {key}: {e}");
                }
                tracing::info!("seed {key}: done");
            }
            Err(e) => tracing::warn!("seed {key}: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn one(state: &Arc<AppState>, pick: &Pick) -> Result<()> {
    let (url, page, died, title) = match pick {
        Pick::Audio(id) => return crate::audio::seed(state, id).await,
        Pick::Gutenberg(id, died) => (
            crate::sources::gutenberg_file(*id),
            format!("https://www.gutenberg.org/ebooks/{id}"),
            Some(*died),
            None,
        ),
        Pick::TextGrid(uri) => (
            crate::open::textgrid_file(uri),
            crate::open::textgrid_page(uri),
            None,
            None,
        ),
        Pick::Wolne { epub, page, .. } => (epub.clone(), page.clone(), None, None),
        Pick::Wikisource {
            lang,
            page,
            title,
            died,
        } => {
            let Some(found) = crate::open::wikisource_page(&state.http, lang, page).await? else {
                return Ok(());
            };
            (
                found.download_url,
                found.page_url,
                Some(*died),
                Some(*title),
            )
        }
    };
    let (stored, _) =
        crate::books::fetch_into_library(state, None, &url, Some(&page), PATIENCE).await?;
    let (Stored::New(id), Some(died)) = (stored, died) else {
        return Ok(());
    };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "update books set title = coalesce($3, title),
             public = extract(year from now())::int - $2 > 70,
             public_basis = case when extract(year from now())::int - $2 > 70
                 then author || ' † ' || $2 end,
             legal_checked_at = now()
         where id = $1",
    )
    .bind(id)
    .bind(died)
    .bind(title)
    .execute(&mut *tx)
    .await?;
    sqlx::query("update book_docs set version = 0 where book_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_picks_mix_languages_and_have_stable_keys() {
        let picks = fixed();
        assert_eq!(
            picks.len(),
            GUTENBERG.len() + TEXTGRID.len() + WIKISOURCE.len()
        );
        assert_eq!(picks[0], Pick::Gutenberg(1342, 1817));
        assert!(matches!(
            picks[1],
            Pick::Wikisource {
                lang: "ru",
                page: "Капитанская дочка (Пушкин)",
                ..
            }
        ));
        assert_eq!(picks[2], Pick::TextGrid("textgrid:11g9q.0"));
        assert!(matches!(picks[3], Pick::Wikisource { lang: "uk", .. }));
        let keys: std::collections::HashSet<String> = picks.iter().map(Pick::key).collect();
        assert_eq!(keys.len(), picks.len());
        assert_eq!(Pick::Gutenberg(84, 1851).key(), "gutenberg:84");
        assert_eq!(Pick::Audio("wl:hamlet".into()).key(), "audio:wl:hamlet");
        let years = GUTENBERG
            .iter()
            .map(|g| g.1)
            .chain(WIKISOURCE.iter().map(|w| w.3));
        assert!(years.into_iter().all(|died| died < 1950));
    }
}
