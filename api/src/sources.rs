//! Поиск книг за пределами своей библиотеки и скачивание найденного.
//!
//! Приоритет у своей библиотеки: сюда фронт идёт вторым запросом, поэтому
//! молчащий или лежащий внешний источник не задерживает выдачу и не ломает её.

use std::time::Duration;

use serde::Serialize;

use crate::{AppError, Result};

/// Куда серверу вообще разрешено ходить за файлом.
///
/// Это защита от SSRF, а не формальность: `/import` скачивает адрес, который
/// прислал клиент, и без списка его можно направить на `169.254.169.254`
/// или в локальную сеть за спиной у файрвола. Проверяется каждый переход
/// по редиректу, а не только первый адрес.
const ALLOWED_HOSTS: &[&str] = &[
    "gutenberg.org",
    "www.gutenberg.org",
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "codeload.github.com",
];

const TIMEOUT: Duration = Duration::from_secs(30);

pub fn host_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
}

pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent("leser/0.1 (+https://leser.cloud)")
        // Редирект — это ещё один запрос, и увести он может куда угодно:
        // gutenberg.org сам редиректит, поэтому запрещать их нельзя, а
        // проверять каждый переход — нужно.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("слишком много редиректов")
            } else if host_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("редирект на посторонний адрес")
            }
        }))
        .build()
        .map_err(AppError::internal)
}

/// Книга, найденная снаружи. В библиотеку она попадает только после явного
/// «добавить»: здесь лежат ссылки, а не файлы.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub title: String,
    pub author: String,
    /// Человекочитаемое имя источника: оно окажется в графе «Источник».
    pub source: String,
    /// Страница, куда ведёт ссылка в подробностях.
    pub page_url: String,
    /// Прямая ссылка на файл — её принимает `/import`.
    pub download_url: String,
    pub ext: String,
}

/// Опрашивает источники параллельно. Упавший источник — это ноль его книг,
/// а не ошибка всего поиска: Gutenberg лежит, GitHub всё равно отвечает.
pub async fn search(client: &reqwest::Client, q: &str, github_token: Option<&str>) -> Vec<Found> {
    let (gutenberg, github) = tokio::join!(gutenberg(client, q), github(client, q, github_token));

    let mut out = Vec::new();
    for (name, result) in [("gutenberg", gutenberg), ("github", github)] {
        match result {
            Ok(mut found) => out.append(&mut found),
            Err(e) => tracing::warn!("источник {name} не ответил: {e}"),
        }
    }
    out
}

// ---------------------------------------------------------------- Gutenberg

/// Project Gutenberg через gutendex.com — открытый API без ключей.
/// Всё, что там лежит, — общественное достояние, поэтому источник первый.
async fn gutenberg(client: &reqwest::Client, q: &str) -> Result<Vec<Found>> {
    #[derive(serde::Deserialize)]
    struct Resp {
        results: Vec<Item>,
    }
    #[derive(serde::Deserialize)]
    struct Item {
        id: u64,
        title: String,
        authors: Vec<Author>,
        formats: std::collections::HashMap<String, String>,
    }
    #[derive(serde::Deserialize)]
    struct Author {
        name: String,
    }

    // слэш обязателен: без него gutendex отвечает редиректом, а не данными
    let resp: Resp = client
        .get("https://gutendex.com/books/")
        .query(&[("search", q)])
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;

    Ok(resp
        .results
        .into_iter()
        .filter_map(|item| {
            // форматов у книги много; нужен epub, и без ".images" в ключе
            // тоже встречается — берём первый подходящий
            let download_url = item
                .formats
                .iter()
                .find(|(k, _)| k.starts_with("application/epub"))
                .map(|(_, v)| v.clone())?;

            Some(Found {
                title: item.title,
                author: item
                    .authors
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default(),
                source: "Project Gutenberg".into(),
                page_url: format!("https://www.gutenberg.org/ebooks/{}", item.id),
                download_url,
                ext: "epub".into(),
            })
        })
        .take(10)
        .collect())
}

// ------------------------------------------------------------------- GitHub

/// Поиск по коду на GitHub находит сами файлы книг, но требует токена:
/// без него `/search/code` отвечает 401. Нет токена — нет источника,
/// остальные при этом работают.
async fn github(client: &reqwest::Client, q: &str, token: Option<&str>) -> Result<Vec<Found>> {
    let Some(token) = token else {
        return Ok(Vec::new());
    };

    #[derive(serde::Deserialize)]
    struct Resp {
        items: Vec<Item>,
    }
    #[derive(serde::Deserialize)]
    struct Item {
        name: String,
        path: String,
        html_url: String,
        repository: Repo,
    }
    #[derive(serde::Deserialize)]
    struct Repo {
        full_name: String,
        default_branch: Option<String>,
    }

    let query = format!("{q} extension:epub extension:fb2");
    let resp: Resp = client
        .get("https://api.github.com/search/code")
        .query(&[("q", query.as_str()), ("per_page", "10")])
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(AppError::internal)?
        .error_for_status()
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;

    Ok(resp
        .items
        .into_iter()
        .filter_map(|item| {
            let ext = item.name.rsplit_once('.')?.1.to_lowercase();
            if !crate::books::BOOK_EXTS.contains(&ext.as_str()) {
                return None;
            }
            let branch = item.repository.default_branch.as_deref().unwrap_or("HEAD");
            Some(Found {
                // в имени файла название книги, метаданные всё равно
                // возьмутся из самого файла при добавлении
                title: item.name.clone(),
                author: String::new(),
                source: format!("GitHub · {}", item.repository.full_name),
                page_url: item.html_url,
                download_url: format!(
                    "https://raw.githubusercontent.com/{}/{}/{}",
                    item.repository.full_name, branch, item.path
                ),
                ext,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_and_only_known_hosts() {
        let ok = |u: &str| host_allowed(&u.parse().unwrap());

        assert!(ok("https://www.gutenberg.org/ebooks/2554.epub3.images"));
        assert!(ok("https://raw.githubusercontent.com/a/b/main/x.epub"));

        // http, даже к разрешённому хосту
        assert!(!ok("http://www.gutenberg.org/x.epub"));
        // адреса, ради которых список и существует
        assert!(!ok("https://169.254.169.254/latest/meta-data/"));
        assert!(!ok("https://localhost/admin"));
        assert!(!ok("https://10.0.0.1/"));
        assert!(!ok("https://evil.com/x.epub"));
        // хост, который лишь заканчивается разрешённым
        assert!(!ok("https://gutenberg.org.evil.com/x.epub"));
        assert!(!ok("https://notgithub.com/x.epub"));
        assert!(!ok("file:///etc/passwd"));
    }
}
