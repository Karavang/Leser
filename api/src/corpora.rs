use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::Deserialize;

use crate::{
    open::{json, names_match, person, starts_like, text, Described},
    sources::Found,
    Result,
};

const ELTEC: [(&str, &str); 15] = [
    ("cze", "cs"),
    ("deu", "de"),
    ("eng", "en"),
    ("fra", "fr"),
    ("fra-ext1", "fr"),
    ("fra-ext2", "fr"),
    ("hun", "hu"),
    ("ita", "it"),
    ("pol", "pl"),
    ("por", "pt"),
    ("rom", "ro"),
    ("slv", "sl"),
    ("spa", "es"),
    ("srp", "sr"),
    ("ukr", "uk"),
];

const DRACOR: &str = "https://dracor.org/api/v1";

const DRACOR_LANGS: [(&str, &str); 12] = [
    ("ar", "ar"),
    ("dutch", "nl"),
    ("ger", "de"),
    ("gersh", "de"),
    ("greek", "grc"),
    ("ibs", "no"),
    ("pol", "pl"),
    ("ro", "ro"),
    ("rom", "la"),
    ("rus", "ru"),
    ("u", "uk"),
    ("yi", "yi"),
];

const FRESH: Duration = Duration::from_secs(24 * 3600);
const RETRY: Duration = Duration::from_secs(600);
const TERM: i32 = 70;

#[derive(Clone)]
struct Work {
    title: String,
    author: String,
    lang: Option<String>,
    source: &'static str,
    page_url: String,
    download_url: String,
    basis: Option<String>,
    hay: String,
}

// ponytail: the whole ELTeC and DraCor catalogue (a few thousand rows) lives in process memory for a day;
// a source that failed is asked again after ten minutes.
static CACHE: Mutex<Option<(Instant, Arc<Vec<Work>>)>> = Mutex::new(None);

async fn works(client: &reqwest::Client) -> Arc<Vec<Work>> {
    if let Some((until, all)) = CACHE.lock().unwrap().as_ref() {
        if Instant::now() < *until {
            return all.clone();
        }
    }
    let year = this_year();
    let (eltec, dracor) = tokio::join!(
        futures_util::future::join_all(
            ELTEC
                .iter()
                .map(|&(repo, lang)| eltec(client, repo, lang, year))
        ),
        dracor(client),
    );
    let mut complete = dracor.is_ok();
    let mut all: Vec<Work> = dracor.unwrap_or_default();
    for found in eltec {
        match found {
            Ok(found) => all.extend(found),
            Err(e) => {
                complete = false;
                tracing::warn!("источник eltec не ответил: {e}");
            }
        }
    }
    let all = Arc::new(all);
    let until = Instant::now() + if complete { FRESH } else { RETRY };
    *CACHE.lock().unwrap() = Some((until, all.clone()));
    all
}

fn this_year() -> i32 {
    sqlx::types::chrono::Utc::now()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(0)
}

async fn eltec(client: &reqwest::Client, repo: &str, lang: &str, year: i32) -> Result<Vec<Work>> {
    let tsv = text(
        client,
        &format!("https://raw.githubusercontent.com/COST-ELTeC/ELTeC-{repo}/HEAD/ELTeC-{repo}_metadata.tsv"),
        &[],
    )
    .await?;
    Ok(eltec_rows(&tsv, repo, lang, year))
}

fn eltec_rows(tsv: &str, repo: &str, lang: &str, year: i32) -> Vec<Work> {
    let mut lines = tsv.lines();
    let head: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (Some(file), Some(author), Some(title)) =
        (col("filename"), col("author-name"), col("title"))
    else {
        return Vec::new();
    };
    let death = col("author-death");
    lines
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('\t').collect();
            let file = cells.get(file)?.trim();
            let title = cells.get(title)?.trim();
            if file.is_empty() || title.is_empty() || !file.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return None;
            }
            let author = person(cells.get(author)?.split('[').next()?.trim());
            let died = death
                .and_then(|d| cells.get(d))
                .and_then(|d| d.trim().parse::<i32>().ok());
            Some(Work {
                basis: died
                    .filter(|d| d + TERM < year)
                    .map(|d| format!("ELTeC: {author} † {d}")),
                hay: format!("{title} {author}"),
                title: title.to_string(),
                author,
                lang: Some(lang.to_string()),
                source: "ELTeC",
                page_url: format!("https://github.com/COST-ELTeC/ELTeC-{repo}/blob/HEAD/level1/{file}.xml"),
                download_url: format!(
                    "https://raw.githubusercontent.com/COST-ELTeC/ELTeC-{repo}/HEAD/level1/{file}.xml"
                ),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct Corpus {
    name: String,
    #[serde(default)]
    licence: Option<String>,
}

#[derive(Deserialize)]
struct Plays {
    #[serde(default)]
    plays: Vec<Play>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Play {
    name: String,
    title: String,
    #[serde(default)]
    title_en: Option<String>,
    #[serde(default)]
    authors: Vec<PlayAuthor>,
}

#[derive(Deserialize)]
struct PlayAuthor {
    #[serde(default)]
    fullname: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "fullnameEn")]
    fullname_en: Option<String>,
}

fn free_licence(licence: &str) -> bool {
    licence.starts_with("CC0")
        || licence.contains("Public Domain")
        || (licence.starts_with("CC BY") && !licence.contains("NC") && !licence.contains("ND"))
}

async fn dracor(client: &reqwest::Client) -> Result<Vec<Work>> {
    let corpora: Vec<Corpus> = json(client, &format!("{DRACOR}/corpora"), &[]).await?;
    let free: Vec<String> = corpora
        .into_iter()
        .filter(|c| c.licence.as_deref().is_some_and(free_licence))
        .map(|c| c.name)
        .filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect();
    let lists = futures_util::future::join_all(free.iter().map(|corpus| async move {
        let plays: Result<Plays> = json(client, &format!("{DRACOR}/corpora/{corpus}"), &[]).await;
        (corpus, plays)
    }))
    .await;
    let mut out = Vec::new();
    for (corpus, plays) in lists {
        let plays = plays?;
        let lang = DRACOR_LANGS
            .iter()
            .find(|(c, _)| c == corpus)
            .map(|(_, l)| l.to_string());
        out.extend(
            plays
                .plays
                .into_iter()
                .filter(|p| crate::open::wl_slug(&p.name))
                .map(|p| {
                    let author = p
                        .authors
                        .iter()
                        .map(|a| a.fullname.clone().unwrap_or_else(|| person(&a.name)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let english = p
                        .authors
                        .iter()
                        .filter_map(|a| a.fullname_en.as_deref())
                        .collect::<Vec<_>>()
                        .join(" ");
                    Work {
                        hay: format!(
                            "{} {} {author} {english}",
                            p.title,
                            p.title_en.as_deref().unwrap_or_default()
                        ),
                        page_url: format!("https://dracor.org/{corpus}/{}", p.name),
                        download_url: format!("{DRACOR}/corpora/{corpus}/plays/{}/tei", p.name),
                        title: p.title,
                        author,
                        lang: lang.clone(),
                        source: "DraCor",
                        basis: None,
                    }
                }),
        );
    }
    Ok(out)
}

pub async fn search(client: &reqwest::Client, q: &str) -> Result<Vec<Found>> {
    let all = works(client).await;
    let mut hits: Vec<&Work> = all.iter().filter(|w| names_match(&w.hay, q)).collect();
    hits.sort_by_key(|w| !starts_like(&w.title, q));
    Ok(hits
        .into_iter()
        .take(8)
        .map(|w| Found {
            title: w.title.clone(),
            author: w.author.clone(),
            source: w.source.into(),
            page_url: w.page_url.clone(),
            download_url: w.download_url.clone(),
            ext: "xml".into(),
            lang: w.lang.clone(),
        })
        .collect())
}

pub async fn describe(client: &reqwest::Client, url: &reqwest::Url) -> Described {
    let all = works(client).await;
    all.iter()
        .find(|w| w.download_url == url.as_str())
        .map(|w| Described {
            title: Some(w.title.clone()),
            author: Some(w.author.clone()).filter(|a| !a.is_empty()),
            basis: w.basis.clone(),
        })
        .unwrap_or_default()
}

pub fn is_tei(url: &reqwest::Url) -> bool {
    url.host_str() == Some("dracor.org") && url.path().ends_with("/tei")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eltec_rows_carry_their_public_domain_basis() {
        let tsv = "corpus-id\tfilename\txmlid\tauthor-name\ttitle\tauthor-birth\tauthor-death\n\
            ELTeC-ukr\tUKR18411_KvitkaSerdeshnaOksana\tUKR18411\tКвітка-Основ'яненко, Григорій\tСердешна Оксана\t1778\t1843\n\
            ELTeC-ukr\tUKR19201_Yatskiv\tx\tЯцків, Михайло\tОповідання\t1873\t1961\n\
            ELTeC-fra\tFRA03704_Sand\tx\tSand, George [Amantine Dupin]\tHorace\t1804\tNA\n\
            ELTeC-ukr\t../evil\tx\ta\tb\t1\t1\n";
        let rows = eltec_rows(tsv, "ukr", "uk", 2026);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].author, "Григорій Квітка-Основ'яненко");
        assert_eq!(
            rows[0].basis.as_deref(),
            Some("ELTeC: Григорій Квітка-Основ'яненко † 1843")
        );
        assert_eq!(
            rows[0].download_url,
            "https://raw.githubusercontent.com/COST-ELTeC/ELTeC-ukr/HEAD/level1/UKR18411_KvitkaSerdeshnaOksana.xml"
        );
        assert_eq!(rows[1].basis, None);
        assert_eq!(rows[2].author, "George Sand");
        assert_eq!(rows[2].basis, None);
    }

    #[test]
    fn only_free_dracor_licences_pass() {
        for ok in ["CC0", "CC BY-SA 4.0", "CC BY-SA 3.0 US", "CC BY 4.0"] {
            assert!(free_licence(ok), "{ok}");
        }
        for no in ["CC BY-NC-SA 4.0", "CC BY-NC 3.0", "CC BY-ND 4.0", ""] {
            assert!(!free_licence(no), "{no}");
        }
    }

    #[tokio::test]
    #[ignore = "goes to dracor.org and github"]
    async fn live_catalogue_finds_and_converts() {
        let client = crate::sources::client().unwrap();
        for q in ["Ревизор", "Сердешна Оксана", "Lalka", "Horace"] {
            let found = search(&client, q).await.unwrap();
            let first = found.first().unwrap_or_else(|| panic!("nothing for {q}"));
            let url: reqwest::Url = first.download_url.parse().unwrap();
            assert!(crate::sources::host_allowed(&url));
            let bytes = client
                .get(url.clone())
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap();
            let tei = crate::tei::read(&bytes).unwrap();
            let fb2 = crate::rda::to_fb2(tei.title.as_deref().unwrap_or(q), "TEI", &tei.chapters);
            crate::parse::parse_fb2(fb2.as_bytes()).unwrap();
            let d = describe(&client, &url).await;
            println!(
                "{q}: {} — {} [{}] {} глав, {:?}",
                first.title,
                first.author,
                first.source,
                tei.chapters.len(),
                d.basis
            );
        }
    }
}
