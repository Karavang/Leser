use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    sources::{strip_tags, Audiobook, Found, FoundAudio, Section},
    AppError, Result,
};

pub(crate) async fn json<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    query: &[(&str, &str)],
) -> Result<T> {
    client
        .get(url)
        .query(query)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)
}

pub(crate) async fn text(client: &reqwest::Client, url: &str, query: &[(&str, &str)]) -> Result<String> {
    client
        .get(url)
        .query(query)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(AppError::internal)?
        .text()
        .await
        .map_err(AppError::internal)
}

fn xml_texts(xml: &str, local: &[u8]) -> Vec<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = Vec::new();
    let mut inside = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => inside = e.local_name().as_ref() == local,
            Ok(Event::Text(t)) if inside => {
                if let Ok(t) = t.unescape() {
                    out.push(t.trim().to_string());
                }
                inside = false;
            }
            Ok(Event::End(_)) => inside = false,
            Ok(Event::Eof) | Err(_) => return out,
            _ => {}
        }
    }
}

const WL: &str = "https://wolnelektury.pl/api/2";

#[derive(Deserialize)]
struct WlPage {
    #[serde(default)]
    member: Vec<WlBook>,
}

#[derive(Deserialize)]
struct WlBook {
    slug: String,
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    authors: Vec<WlName>,
    #[serde(default)]
    parent: Option<serde_json::Value>,
    #[serde(default)]
    preview: bool,
    #[serde(default)]
    epub: Option<String>,
    #[serde(default)]
    has_mp3_file: bool,
    #[serde(default, rename = "abstract")]
    summary: String,
}

#[derive(Deserialize)]
struct WlName {
    name: String,
}

#[derive(Deserialize)]
struct WlSearch {
    #[serde(default)]
    author: Vec<WlTag>,
}

#[derive(Deserialize)]
struct WlTag {
    id: u64,
    #[serde(default)]
    name: String,
}

pub fn names_match(name: &str, q: &str) -> bool {
    let name = letters(name);
    q.split_whitespace().all(|w| name.contains(&letters(w)))
}

#[derive(Deserialize)]
struct WlTrack {
    url: String,
    #[serde(default)]
    part_name: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    director: String,
    #[serde(default)]
    duration: f64,
}

fn names(list: &[WlName]) -> String {
    list.iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

async fn wl_books(client: &reqwest::Client, q: &str, audio: bool) -> Result<Vec<WlBook>> {
    let has_audio = if audio { "true" } else { "" };
    let by_title = async {
        let mut query = vec![("search", q), ("limit", "20")];
        if audio {
            query.push(("has_audio", has_audio));
        }
        json::<WlPage>(client, &format!("{WL}/books/"), &query).await
    };
    let by_author = async {
        let found: WlSearch = json(client, &format!("{WL}/search/"), &[("q", q)]).await?;
        let Some(author) = found.author.iter().find(|a| names_match(&a.name, q)) else {
            return Ok(WlPage { member: Vec::new() });
        };
        let tag = author.id.to_string();
        let mut query = vec![("tag", tag.as_str()), ("limit", "20")];
        if audio {
            query.push(("has_audio", has_audio));
        }
        json::<WlPage>(client, &format!("{WL}/books/"), &query).await
    };
    let (by_title, by_author) = tokio::join!(by_title, by_author);
    let mut seen = HashSet::new();
    Ok(by_title?
        .member
        .into_iter()
        .chain(by_author.map(|p| p.member).unwrap_or_default())
        .filter(|b| b.parent.as_ref().is_none_or(serde_json::Value::is_null) && !b.preview)
        .filter(|b| seen.insert(b.slug.clone()))
        .collect())
}

pub async fn wolnelektury(client: &reqwest::Client, q: &str) -> Result<Vec<Found>> {
    Ok(wl_books(client, q, false)
        .await?
        .into_iter()
        .filter_map(|b| {
            Some(Found {
                download_url: b.epub.clone()?,
                author: names(&b.authors),
                source: "Wolne Lektury".into(),
                page_url: b.url,
                ext: "epub".into(),
                lang: crate::audio::lang_code(Some(&b.language)),
                title: b.title,
            })
        })
        .take(6)
        .collect())
}

pub async fn wolnelektury_audio(client: &reqwest::Client, q: &str) -> Result<Vec<FoundAudio>> {
    Ok(wl_books(client, q, true)
        .await?
        .into_iter()
        .filter(|b| b.has_mp3_file)
        .take(8)
        .map(|b| FoundAudio {
            id: format!("wl:{}", b.slug),
            author: names(&b.authors),
            seconds: 0,
            lang: crate::audio::lang_code(Some(&b.language)),
            source: "Wolne Lektury".into(),
            page_url: b.url,
            title: b.title,
        })
        .collect())
}

async fn wl_popular(client: &reqwest::Client, audio: bool) -> Result<Vec<WlBook>> {
    let mut query = vec![("sort", "-popularity"), ("limit", "40")];
    if audio {
        query.push(("has_audio", "true"));
    }
    let page: WlPage = json(client, &format!("{WL}/books/"), &query).await?;
    Ok(page
        .member
        .into_iter()
        .filter(|b| b.parent.as_ref().is_none_or(serde_json::Value::is_null) && !b.preview)
        .collect())
}

pub async fn wolnelektury_top(client: &reqwest::Client, n: usize) -> Result<Vec<Found>> {
    Ok(wl_popular(client, false)
        .await?
        .into_iter()
        .filter_map(|b| {
            Some(Found {
                download_url: b.epub.clone()?,
                author: names(&b.authors),
                source: "Wolne Lektury".into(),
                page_url: b.url,
                ext: "epub".into(),
                lang: crate::audio::lang_code(Some(&b.language)),
                title: b.title,
            })
        })
        .take(n)
        .collect())
}

pub async fn wolnelektury_audio_top(
    client: &reqwest::Client,
    n: usize,
    max_bytes: u64,
) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct Sized {
        #[serde(default)]
        size: u64,
    }
    let mut out = Vec::new();
    for book in wl_popular(client, true).await? {
        if out.len() >= n {
            break;
        }
        if !book.has_mp3_file {
            continue;
        }
        let tracks: Vec<Sized> =
            json(client, &format!("{WL}/books/{}/media/mp3/", book.slug), &[]).await?;
        let total: u64 = tracks.iter().map(|t| t.size).sum();
        if !tracks.is_empty() && total <= max_bytes {
            out.push(format!("wl:{}", book.slug));
        }
    }
    Ok(out)
}

pub fn wl_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

async fn wl_rights(client: &reqwest::Client, slug: &str) -> Option<String> {
    let id = format!("oai:wolnelektury.pl:{slug}");
    let xml = text(
        client,
        "https://wolnelektury.pl/oaipmh/",
        &[
            ("verb", "GetRecord"),
            ("metadataPrefix", "oai_dc"),
            ("identifier", &id),
        ],
    )
    .await
    .map_err(|e| tracing::warn!("wolnelektury rights of {slug}: {e}"))
    .ok()?;
    xml_texts(&xml, b"rights")
        .into_iter()
        .find(|r| !r.is_empty())
}

fn unique(list: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    list.filter(|s| !s.trim().is_empty() && seen.insert(s.clone()))
        .collect()
}

pub async fn wolnelektury_book(client: &reqwest::Client, slug: &str) -> Result<Audiobook> {
    let book: WlBook = json(client, &format!("{WL}/books/{slug}/"), &[]).await?;
    let tracks: Vec<WlTrack> = json(client, &format!("{WL}/books/{slug}/media/mp3/"), &[]).await?;
    let rights = wl_rights(client, slug).await;

    let readers = unique(tracks.iter().map(|t| t.artist.clone()));
    let directors = unique(tracks.iter().map(|t| t.director.clone()));
    let mut credits = Vec::new();
    if !readers.is_empty() {
        credits.push(format!("Czyta: {}.", readers.join(", ")));
    }
    if !directors.is_empty() {
        credits.push(format!("Reżyseria: {}.", directors.join(", ")));
    }
    let summary = strip_tags(&book.summary).trim().to_string();
    let description = [summary, credits.join(" ")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(Audiobook {
        author: names(&book.authors),
        lang: crate::audio::lang_code(Some(&book.language)),
        description: (!description.is_empty()).then_some(description),
        page_url: book.url,
        title: book.title,
        basis: Some(basis_wl(rights.as_deref())),
        sections: tracks
            .into_iter()
            .map(|t| Section {
                title: t.part_name,
                url: t.url,
                seconds: t.duration,
            })
            .collect(),
    })
}

fn basis_wl(rights: Option<&str>) -> String {
    match rights {
        Some(r) => format!("Wolne Lektury: {r}"),
        None => "Wolne Lektury".into(),
    }
}

fn cyrillic(q: &str) -> bool {
    q.chars().any(|c| matches!(c, '\u{0400}'..='\u{04ff}'))
}

fn ws_langs(q: &str) -> &'static [&'static str] {
    let has = |lo: char, hi: char| q.chars().any(|c| (lo..=hi).contains(&c));
    if cyrillic(q) {
        &["ru", "uk", "be", "bg", "sr"]
    } else if has('\u{3040}', '\u{30ff}') {
        &["ja"]
    } else if has('\u{ac00}', '\u{d7af}') {
        &["ko"]
    } else if has('\u{4e00}', '\u{9fff}') {
        &["zh", "ja"]
    } else if has('\u{0370}', '\u{03ff}') {
        &["el"]
    } else if has('\u{0590}', '\u{05ff}') {
        &["he"]
    } else if has('\u{0600}', '\u{06ff}') {
        &["ar", "fa"]
    } else if has('\u{0530}', '\u{058f}') {
        &["hy"]
    } else if has('\u{10a0}', '\u{10ff}') {
        &["ka"]
    } else if has('\u{0900}', '\u{097f}') {
        &["hi"]
    } else if has('\u{0980}', '\u{09ff}') {
        &["bn"]
    } else if has('\u{0b80}', '\u{0bff}') {
        &["ta"]
    } else if has('\u{0e00}', '\u{0e7f}') {
        &["th"]
    } else {
        &["en", "de", "pl", "fr", "es", "it", "pt", "cs", "nl", "sv", "la"]
    }
}

#[derive(Deserialize, Default)]
struct WsAnswer {
    #[serde(default)]
    query: WsQuery,
}

#[derive(Deserialize, Default)]
struct WsQuery {
    #[serde(default)]
    prefixsearch: Vec<WsTitle>,
    #[serde(default)]
    pages: Vec<WsPage>,
    #[serde(default)]
    redirects: Vec<WsRedirect>,
    #[serde(default)]
    normalized: Vec<WsRedirect>,
}

#[derive(Deserialize)]
struct WsTitle {
    title: String,
}

#[derive(Deserialize)]
struct WsRedirect {
    from: String,
    to: String,
}

#[derive(Deserialize)]
struct WsPage {
    title: String,
    #[serde(default)]
    missing: bool,
    #[serde(default)]
    pageprops: Option<serde_json::Value>,
    #[serde(default)]
    categories: Vec<WsTitle>,
}

impl WsPage {
    fn lists_versions(&self) -> bool {
        self.pageprops
            .as_ref()
            .is_some_and(|p| p.get("disambiguation").is_some())
            || self.categories.iter().any(|c| {
                let c = c.title.to_lowercase();
                c.contains("списки редакций") || c.contains("сторінки версій")
            })
    }
}

pub fn roots(titles: &[String], limit: usize) -> Vec<String> {
    unique(
        titles
            .iter()
            .map(|t| t.split('/').next().unwrap_or(t).trim().to_string()),
    )
    .into_iter()
    .take(limit)
    .collect()
}

fn letters(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn starts_like(title: &str, q: &str) -> bool {
    letters(title).starts_with(&letters(q))
}

pub fn editions(base: &str, links: &[String], limit: usize) -> Vec<String> {
    let key: String = letters(base).chars().take(6).collect();
    let depth = base.matches('/').count();
    let rank = |l: &str| {
        let last = l.rsplit('/').next().unwrap_or(l);
        if l.starts_with(&format!("{base}/")) && l.matches('/').count() == depth + 1 {
            Some(0)
        } else if letters(l).starts_with(&key) && l.matches('/').count() <= depth + 1 {
            Some(1)
        } else if last != l && letters(last).starts_with(&key) {
            Some(2)
        } else {
            None
        }
    };
    let mut found: Vec<(u8, &String)> = links
        .iter()
        .filter_map(|l| rank(l).map(|r| (r, l)))
        .collect();
    found.sort_by_key(|(r, l)| (*r, l.contains("(ДО)")));
    found
        .into_iter()
        .take(limit)
        .map(|(_, l)| l.clone())
        .collect()
}

async fn ws_query(
    client: &reqwest::Client,
    lang: &str,
    params: &[(&str, &str)],
) -> Result<WsQuery> {
    let mut query = vec![
        ("action", "query"),
        ("format", "json"),
        ("formatversion", "2"),
    ];
    query.extend_from_slice(params);
    let answer: WsAnswer = json(
        client,
        &format!("https://{lang}.wikisource.org/w/api.php"),
        &query,
    )
    .await?;
    Ok(answer.query)
}

pub fn ws_export(lang: &str, page: &str) -> String {
    reqwest::Url::parse_with_params(
        "https://ws-export.wmcloud.org/",
        &[
            ("format", "epub-3"),
            ("lang", lang),
            ("page", page),
            ("fonts", "none"),
        ],
    )
    .map(String::from)
    .unwrap_or_default()
}

// ponytail: languages are picked by the query's script, and the calls run one after another,
// as the Wikimedia robot policy asks of unauthenticated clients; whatever has not answered
// within WS_BUDGET is left out, so the Latin list beyond the first few languages is best effort.
const WS_BUDGET: std::time::Duration = std::time::Duration::from_secs(8);

pub async fn wikisource(client: &reqwest::Client, q: &str) -> Result<Vec<Found>> {
    let deadline = tokio::time::Instant::now() + WS_BUDGET;
    let mut lists = Vec::new();
    for lang in ws_langs(q) {
        match tokio::time::timeout_at(deadline, wikisource_in(client, lang, q)).await {
            Ok(Ok(found)) => lists.push(found),
            Ok(Err(e)) => tracing::warn!("источник wikisource {lang} не ответил: {e}"),
            Err(_) => break,
        }
    }
    Ok(crate::sources::interleave(lists))
}

async fn wikisource_in(client: &reqwest::Client, lang: &str, q: &str) -> Result<Vec<Found>> {
    let found = ws_query(
        client,
        lang,
        &[
            ("list", "prefixsearch"),
            ("pssearch", q),
            ("pslimit", "20"),
            ("psnamespace", "0"),
        ],
    )
    .await?;
    let titles: Vec<String> = found
        .prefixsearch
        .into_iter()
        .map(|t| t.title)
        .filter(|t| starts_like(t, q))
        .collect();
    let roots = roots(&titles, 6);
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    Ok(works(client, lang, &roots)
        .await?
        .into_iter()
        .take(4)
        .map(|title| ws_found(lang, title))
        .collect())
}

pub async fn wikisource_page(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
) -> Result<Option<Found>> {
    Ok(works(client, lang, &[title.to_string()])
        .await?
        .into_iter()
        .next()
        .map(|t| ws_found(lang, t)))
}

fn ws_found(lang: &str, title: String) -> Found {
    Found {
        page_url: format!(
            "https://{lang}.wikisource.org/wiki/{}",
            percent_encoding::utf8_percent_encode(
                &title.replace(' ', "_"),
                percent_encoding::NON_ALPHANUMERIC
            )
        ),
        download_url: ws_export(lang, &title),
        author: String::new(),
        source: "Wikisource".into(),
        ext: "epub".into(),
        lang: Some(lang.to_string()),
        title,
    }
}

async fn works(client: &reqwest::Client, lang: &str, roots: &[String]) -> Result<Vec<String>> {
    let joined = roots.join("|");
    let props = ws_query(
        client,
        lang,
        &[
            ("titles", &joined),
            ("redirects", "1"),
            ("prop", "pageprops|categories"),
            ("ppprop", "disambiguation"),
            ("cllimit", "max"),
        ],
    )
    .await?;
    let renamed: HashMap<String, String> = props
        .normalized
        .into_iter()
        .chain(props.redirects)
        .map(|r| (r.from, r.to))
        .collect();
    let pages: HashMap<String, WsPage> = props
        .pages
        .into_iter()
        .filter(|p| !p.missing)
        .map(|p| (p.title.clone(), p))
        .collect();

    let versions: Vec<&str> = pages
        .values()
        .filter(|p| p.lists_versions())
        .map(|p| p.title.as_str())
        .collect();
    let mut linked: Vec<String> = Vec::new();
    if !versions.is_empty() {
        let joined = versions.join("|");
        linked = ws_query(
            client,
            lang,
            &[
                ("titles", &joined),
                ("generator", "links"),
                ("gplnamespace", "0"),
                ("gpllimit", "max"),
                ("prop", "info"),
            ],
        )
        .await?
        .pages
        .into_iter()
        .filter(|p| !p.missing)
        .map(|p| p.title)
        .collect();
    }

    let mut works = Vec::new();
    for root in roots {
        let mut title = root.clone();
        while let Some(next) = renamed.get(&title) {
            if *next == title {
                break;
            }
            title = next.clone();
        }
        let Some(page) = pages.get(&title) else {
            continue;
        };
        if page.lists_versions() {
            works.extend(editions(&page.title, &linked, 3));
        } else {
            works.push(page.title.clone());
        }
    }

    Ok(unique(works.into_iter()))
}

const TEXTGRID: &str = "https://textgridlab.org/1.0";
const TG_PROJECT: &str = "TGPR-372fe6dc-57f2-6cd4-01b5-2c4bbefcfd3c";
const TG_MIN_SIZE: u64 = 50_000;

#[derive(Debug, Default, PartialEq)]
pub struct TgHit {
    pub author: String,
    pub title: String,
    pub uri: String,
    pub size: u64,
}

pub fn tg_hits(xml: &str) -> Vec<TgHit> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = Vec::new();
    let mut hit: Option<TgHit> = None;
    let mut field: Option<Vec<u8>> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_vec();
                if name == b"result" {
                    let author = e
                        .try_get_attribute("author")
                        .ok()
                        .flatten()
                        .and_then(|a| {
                            a.decode_and_unescape_value(reader.decoder())
                                .ok()
                                .map(|v| v.into_owned())
                        })
                        .unwrap_or_default();
                    hit = Some(TgHit {
                        author,
                        ..Default::default()
                    });
                } else {
                    field = Some(name);
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(h), Some(f), Ok(v)) = (hit.as_mut(), field.as_deref(), t.unescape()) {
                    let v = v.trim().to_string();
                    match f {
                        b"title" if h.title.is_empty() => h.title = v,
                        b"textgridUri" => h.uri = v,
                        b"extent" => h.size = v.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                field = None;
                if e.local_name().as_ref() == b"result" {
                    out.extend(hit.take());
                }
            }
            Ok(Event::Eof) | Err(_) => return out,
            _ => {}
        }
    }
}

pub fn person(name: &str) -> String {
    match name.split_once(", ") {
        Some((last, first)) if !first.contains(',') => format!("{first} {last}"),
        _ => name.to_string(),
    }
}

pub fn textgrid_file(uri: &str) -> String {
    format!("{TEXTGRID}/aggregator/epub/{uri}")
}

pub fn textgrid_page(uri: &str) -> String {
    format!("https://textgridrep.org/{uri}")
}

fn tg_uri(s: &str) -> bool {
    s.strip_prefix("textgrid:").is_some_and(|rest| {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
    })
}

pub async fn textgrid(client: &reqwest::Client, q: &str) -> Result<Vec<Found>> {
    let words: String = q
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if words.is_empty() || cyrillic(&words) {
        return Ok(Vec::new());
    }
    let query = format!("title:({words}) OR edition.agent.value:({words})");
    let project = format!("project.id:{TG_PROJECT}");
    let xml = text(
        client,
        &format!("{TEXTGRID}/tgsearch-public/search"),
        &[
            ("q", &query),
            ("filter", &project),
            ("filter", "format:text/xml"),
            ("limit", "100"),
            ("kwicWidth", "1"),
        ],
    )
    .await?;
    Ok(tg_hits(&xml)
        .into_iter()
        .filter(|h| h.size >= TG_MIN_SIZE && tg_uri(&h.uri) && !h.title.starts_with("Biographie"))
        .take(5)
        .map(|h| Found {
            page_url: textgrid_page(&h.uri),
            download_url: textgrid_file(&h.uri),
            author: person(&h.author),
            source: "TextGrid".into(),
            ext: "epub".into(),
            lang: Some("de".into()),
            title: h.title,
        })
        .collect())
}

#[derive(Debug, Default, PartialEq)]
pub struct Described {
    pub title: Option<String>,
    pub author: Option<String>,
    pub basis: Option<String>,
}

pub async fn describe(client: &reqwest::Client, url: &reqwest::Url) -> Described {
    let path = url.path();
    match url.host_str() {
        Some("wolnelektury.pl") => {
            let Some(slug) = path
                .strip_prefix("/media/book/epub/")
                .and_then(|f| f.strip_suffix(".epub"))
                .filter(|s| wl_slug(s))
            else {
                return Described::default();
            };
            Described {
                basis: wl_rights(client, slug).await.map(|r| basis_wl(Some(&r))),
                ..Default::default()
            }
        }
        Some("textgridlab.org") => {
            let Some(uri) = path
                .strip_prefix("/1.0/aggregator/epub/")
                .filter(|u| tg_uri(u))
            else {
                return Described::default();
            };
            let query = format!("textgridUri:\"{uri}\"");
            let hit = text(
                client,
                &format!("{TEXTGRID}/tgsearch-public/search"),
                &[("q", &query), ("limit", "1"), ("kwicWidth", "1")],
            )
            .await
            .map_err(|e| tracing::warn!("textgrid metadata of {uri}: {e}"))
            .ok()
            .and_then(|xml| tg_hits(&xml).into_iter().next())
            .unwrap_or_default();
            Described {
                title: Some(hit.title).filter(|t| !t.is_empty()),
                author: Some(hit.author)
                    .filter(|a| !a.is_empty())
                    .map(|a| person(&a)),
                basis: Some("TextGrid Repository, CC BY 3.0 DE (Editura / zeno.org)".into()),
            }
        }
        Some("dracor.org" | "raw.githubusercontent.com") => crate::corpora::describe(client, url).await,
        _ => Described::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn chapters_fold_into_their_work() {
        let found = s(&[
            "Капитанская дочка",
            "Капитанская дочка (Пушкин)/1978 (СО)",
            "Капитанская дочка (Пушкин)/1960 (СО)/Глава VI",
            "Война и мир (Толстой)/Том 1",
            "Война и мир (Толстой)",
        ]);
        assert_eq!(
            roots(&found, 6),
            s(&[
                "Капитанская дочка",
                "Капитанская дочка (Пушкин)",
                "Война и мир (Толстой)"
            ])
        );
        assert_eq!(roots(&found, 1).len(), 1);
    }

    #[test]
    fn version_lists_open_into_editions_not_critique_or_chapters() {
        let links = s(&[
            "Капитанская дочка (Пушкин)/1960 (СО)",
            "Капитанская дочка (Пушкин)/1960 (СО)/Глава I",
            "Пиковая дама (Пушкин)",
            "Капитанская дочка (Пушкин)/1978 (СО)",
        ]);
        let mut links = links;
        links.insert(0, "«Капитанская дочка» Пушкина (Черняев)".into());
        assert_eq!(
            editions("Капитанская дочка (Пушкин)", &links, 3),
            s(&[
                "Капитанская дочка (Пушкин)/1960 (СО)",
                "Капитанская дочка (Пушкин)/1978 (СО)",
                "«Капитанская дочка» Пушкина (Черняев)",
            ])
        );
        assert_eq!(
            editions(
                "Захар Беркут",
                &s(&[
                    "Твори (Франко, 1956–1962)/13",
                    "Твори (Франко, 1956–1962)/13/Захар Беркут"
                ]),
                3
            ),
            s(&["Твори (Франко, 1956–1962)/13/Захар Беркут"])
        );
        assert!(starts_like("Lalka (Prus)", "lalka"));
        assert!(!starts_like("Lalla Rookh", "Lalka"));
        assert!(starts_like("Кайдашева сімя̀", "Кайдашева"));
        assert!(names_match("Bolesław Prus", "prus"));
        assert!(names_match("Bolesław Prus", "Bolesław  Prus"));
        assert!(!names_match("Tadeusz Borowski", "Pan Tadeusz"));
        assert_eq!(
            editions(
                "Кайдашева сім'я",
                &s(&["Кайдашева сімя̀", "Іван Нечуй-Левицький"]),
                3
            ),
            s(&["Кайдашева сімя̀"])
        );
    }

    #[test]
    fn ws_export_link_is_encoded() {
        let url = ws_export("ru", "Капитанская дочка (Пушкин)/1960 (СО)");
        assert!(url.starts_with("https://ws-export.wmcloud.org/?format=epub-3&lang=ru&page=%D0%9A"));
        assert!(url.ends_with("&fonts=none"));
        assert!(crate::sources::host_allowed(&url.parse().unwrap()));
    }

    #[test]
    fn textgrid_results_are_read_from_xml() {
        let xml = r#"<?xml version="1.0"?><tgs:response xmlns="http://textgrid.info/namespaces/metadata/core/2010" xmlns:tgs="http://www.textgrid.info/namespaces/middleware/tgsearch" hits="2"><tgs:result author="Fontane, Theodor" score="3"><object><generic><provided><title>Effi Briest</title><format>text/xml</format></provided><generated><textgridUri extRef="">textgrid:n18v.0</textgridUri><extent>791049</extent></generated></generic></object><tgs:fulltext><tgs:kwic><tgs:left>a</tgs:left><tgs:match>Effi</tgs:match></tgs:kwic></tgs:fulltext></tgs:result><tgs:result author="Goethe, Johann Wolfgang von" score="1"><object><generic><provided><title>An Werther &amp; Lotte</title></provided><generated><textgridUri>textgrid:11g3c.0</textgridUri><extent>10775</extent></generated></generic></object></tgs:result></tgs:response>"#;
        assert_eq!(
            tg_hits(xml),
            vec![
                TgHit {
                    author: "Fontane, Theodor".into(),
                    title: "Effi Briest".into(),
                    uri: "textgrid:n18v.0".into(),
                    size: 791_049,
                },
                TgHit {
                    author: "Goethe, Johann Wolfgang von".into(),
                    title: "An Werther & Lotte".into(),
                    uri: "textgrid:11g3c.0".into(),
                    size: 10_775,
                },
            ]
        );
        assert_eq!(
            person("Goethe, Johann Wolfgang von"),
            "Johann Wolfgang von Goethe"
        );
        assert_eq!(person("Homer"), "Homer");
        assert!(tg_uri("textgrid:n18v.0"));
        assert!(!tg_uri("textgrid:../x"));
        assert!(!tg_uri("n18v.0"));
    }

    #[test]
    fn rights_come_out_of_oai_dc() {
        let xml = r#"<OAI-PMH><GetRecord><record><metadata><oai_dc:dc xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Prus, Bolesław</dc:creator><dc:rights>Domena publiczna - Bolesław Prus zm. 1912</dc:rights></oai_dc:dc></metadata></record></GetRecord></OAI-PMH>"#;
        assert_eq!(
            xml_texts(xml, b"rights"),
            vec!["Domena publiczna - Bolesław Prus zm. 1912"]
        );
        assert_eq!(basis_wl(None), "Wolne Lektury");
        assert!(wl_slug("20-000-mil-podmorskiej-zeglugi"));
        assert!(!wl_slug("../etc"));
        assert!(!wl_slug(""));
    }

    #[test]
    fn script_picks_the_wikisource_languages() {
        assert_eq!(ws_langs("Кайдашева")[..3], ["ru", "uk", "be"]);
        assert_eq!(ws_langs("Lalka")[0], "en");
        assert_eq!(ws_langs("吾輩は猫である"), ["ja"]);
        assert_eq!(ws_langs("紅樓夢"), ["zh", "ja"]);
        assert_eq!(ws_langs("Ὀδύσσεια"), ["el"]);
    }

    #[tokio::test]
    #[ignore = "goes to wikisource"]
    async fn live_wikisource_answers_in_time() {
        let client = crate::sources::client().unwrap();
        for q in ["Новая зямля", "Кобзар", "吾輩は猫である", "Οδύσσεια", "Les Misérables", "Don Quijote"] {
            let at = std::time::Instant::now();
            let found = wikisource(&client, q).await.unwrap();
            let langs: Vec<_> = found.iter().filter_map(|f| f.lang.clone()).collect();
            println!("{q}: {} за {:?} {langs:?}", found.len(), at.elapsed());
            assert!(at.elapsed() < std::time::Duration::from_secs(9));
        }
    }
}
