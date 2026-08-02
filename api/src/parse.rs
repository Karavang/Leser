//! Метаданные книги из байтов в памяти. Ни временных файлов, ни диска —
//! в Node-версии epub писался на диск и удалялся до конца разбора.

use quick_xml::events::Event;

use crate::{AppError, Result};

#[derive(Debug, Default, PartialEq)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub series: Option<String>,
    pub published: Option<String>,
    pub lang: Option<String>,
    pub description: Option<String>,
}

// Потолки на длину полей. Заголовок в 60 мегабайт — это не заголовок, а способ
// раздуть таблицу и заставить /getAll отдавать это каждому читателю.
const TITLE_MAX: usize = 300;
const AUTHOR_MAX: usize = 300;
const SERIES_MAX: usize = 200;
const PUBLISHED_MAX: usize = 60;
const LANG_MAX: usize = 30;
const DESCRIPTION_MAX: usize = 4000;

/// Чистит одно поле метаданных и обрезает по длине.
///
/// SQL-инъекции здесь ни при чём: sqlx отправляет значения отдельно от текста
/// запроса (`.bind`), сервер БД их как SQL не разбирает вообще. Чистим от того,
/// что Postgres физически не хранит, и от того, что портит вид списка книг.
fn clean(s: &str, limit: usize) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| {
            // \0 в text Postgres не кладётся — сейчас это была бы ошибка 500
            // на ровном месте. Остальные control-символы убираем заодно,
            // \n и \t оставляем: в аннотации это абзацы.
            (!c.is_control() || *c == '\n' || *c == '\t')
                // U+202E и соседи разворачивают текст задом наперёд: ими
                // название в списке маскируют под что угодно другое.
                && !matches!(c,
                    '\u{200e}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect();

    let trimmed = cleaned.trim();
    // Режем по границе символа: срез по байту посреди utf-8 паникует.
    match trimmed.char_indices().nth(limit) {
        Some((i, _)) => format!("{}…", trimmed[..i].trim_end()),
        None => trimmed.to_string(),
    }
}

impl Meta {
    /// Единственная дверь, через которую метаданные попадают в БД: и epub,
    /// и fb2 выходят только отсюда. Пустые поля становятся `None`, чтобы
    /// в списке не мелькали пустые строки вместо отсутствующих данных.
    pub(crate) fn sanitized(mut self) -> Self {
        let opt = |v: Option<String>, limit| v.map(|s| clean(&s, limit)).filter(|s| !s.is_empty());
        self.title = clean(&self.title, TITLE_MAX);
        self.author = clean(&self.author, AUTHOR_MAX);
        self.series = opt(self.series, SERIES_MAX);
        self.published = opt(self.published, PUBLISHED_MAX);
        self.lang = opt(self.lang, LANG_MAX);
        self.description = opt(self.description, DESCRIPTION_MAX);

        if self.title.is_empty() {
            self.title = UNTITLED.into();
        }
        self
    }
}

/// Название, которого в файле не нашлось. Для txt, pdf и html это норма —
/// `store_book` подставляет вместо него имя файла.
pub const UNTITLED: &str = "Untitled";

/// Разбор синхронный и CPU-bound (распаковка zip), поэтому уходит в blocking-пул.
pub async fn metadata(ext: &str, bytes: axum::body::Bytes) -> Result<Meta> {
    let ext = ext.to_string();
    tokio::task::spawn_blocking(move || match ext.as_str() {
        "epub" => parse_epub(&bytes),
        "fb2" => parse_fb2(&bytes),
        "mobi" | "azw3" => crate::mobi::meta(&bytes),
        "html" => Ok(parse_html(&bytes)),
        "md" => Ok(parse_md(&bytes)),
        // В txt метаданных нет вовсе, а в pdf они есть, но обычно врут:
        // «Microsoft Word - doc1» и имя того, кто печатал. Имя файла честнее.
        "txt" | "pdf" => Ok(Meta::default().sanitized()),
        _ => Err(AppError::bad(
            "Неизвестный формат файла",
            "Invalid file type",
        )),
    })
    .await
    .map_err(AppError::internal)?
}

/// html: всё, что в нём бывает про книгу, — это `<title>`. Автора в `<meta>`
/// пишут единицы, и что там окажется — угадать нельзя.
fn parse_html(bytes: &[u8]) -> Meta {
    let text = crate::reader::decode(bytes, None);
    let title = ["<title>", "<TITLE>"]
        .iter()
        .find_map(|open| text.split_once(open))
        // до ближайшего тега — то есть до </title>
        .and_then(|(_, rest)| rest.split('<').next())
        .unwrap_or_default();

    Meta {
        title: title.to_string(),
        ..Default::default()
    }
    .sanitized()
}

/// markdown: названием служит первый заголовок — другого места под него
/// в формате нет.
fn parse_md(bytes: &[u8]) -> Meta {
    let text = crate::reader::decode(bytes, None);
    let title = text
        .lines()
        .take(50)
        .find(|l| l.trim_start().starts_with('#'))
        .map(|l| l.trim().trim_start_matches('#').trim())
        .unwrap_or_default();

    Meta {
        title: title.to_string(),
        ..Default::default()
    }
    .sanitized()
}

fn parse_epub(bytes: &[u8]) -> Result<Meta> {
    let doc = epub::doc::EpubDoc::from_reader(std::io::Cursor::new(bytes)).map_err(|e| {
        AppError::bad(
            format!("Файл epub повреждён: {e}"),
            format!("Broken epub: {e}"),
        )
    })?;
    let get = |k: &str| {
        doc.mdata(k)
            .map(|m| m.value.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    Ok(Meta {
        title: get("title").unwrap_or_default(),
        author: get("creator").unwrap_or_default(),
        // EPUB3 пишет серию в belongs-to-collection, calibre в EPUB2 —
        // в <meta name="calibre:series">. Встречается и то, и другое.
        series: get("belongs-to-collection").or_else(|| get("calibre:series")),
        published: get("date"),
        lang: get("language"),
        description: get("description"),
    }
    .sanitized())
}

/// FB2 читаем потоково и обрываемся на `</description>` — тело книги
/// (часто с картинками в base64 на десятки мегабайт) не парсится вовсе.
pub(crate) fn parse_fb2(bytes: &[u8]) -> Result<Meta> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut meta = Meta::default();
    let mut authors: Vec<String> = Vec::new();
    let (mut first, mut last) = (String::new(), String::new());
    let mut annotation = String::new();
    let mut closed = false;

    let local = |name: &[u8]| {
        String::from_utf8_lossy(name.rsplit(|c| *c == b':').next().unwrap_or(name)).into_owned()
    };
    // <sequence name="Гарри Поттер" number="2"/> — серия лежит в атрибуте,
    // а не в тексте, поэтому мимо разбора Event::Text она бы проехала.
    // decoder берём заранее: он Copy, а тянуть в замыкание сам reader нельзя —
    // ниже он одалживается на чтение событий.
    let decoder = reader.decoder();
    let sequence_name = |e: &quick_xml::events::BytesStart| {
        e.attributes()
            .flatten()
            .find(|a| local(a.key.as_ref()) == "name")
            .and_then(|a| a.decode_and_unescape_value(decoder).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };

    // Всё, что нам нужно, лежит в <description><title-info>: document-info
    // с данными оцифровщика к самой книге отношения не имеет.
    let in_title_info = |stack: &[String]| stack.iter().any(|s| s == "title-info");
    let at = |stack: &[String], tail: &[&str]| {
        stack.len() >= tail.len()
            && stack
                .iter()
                .rev()
                .zip(tail.iter().rev())
                .all(|(a, b)| a == b)
            && in_title_info(stack)
    };

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let name = local(e.name().as_ref());
                // серия может прийти и парным тегом, и пустым — ловим оба
                if name == "sequence" && meta.series.is_none() && in_title_info(&stack) {
                    meta.series = sequence_name(&e);
                }
                stack.push(name);
            }
            Event::Empty(e) => {
                let name = local(e.name().as_ref());
                if name == "description" {
                    closed = true;
                    break;
                }
                if name == "sequence" && meta.series.is_none() && in_title_info(&stack) {
                    meta.series = sequence_name(&e);
                }
            }
            Event::Text(t) => {
                let text = t.unescape()?.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if at(&stack, &["book-title"]) {
                    meta.title = text;
                } else if at(&stack, &["author", "first-name"]) {
                    first = text;
                // ник заменяет фамилию, только если имени с фамилией вообще нет
                } else if at(&stack, &["author", "last-name"])
                    || (at(&stack, &["author", "nickname"]) && first.is_empty() && last.is_empty())
                {
                    last = text;
                } else if at(&stack, &["date"]) {
                    meta.published = Some(text);
                } else if at(&stack, &["lang"]) {
                    meta.lang = Some(text);
                } else if stack.iter().any(|s| s == "annotation") {
                    if !annotation.is_empty() {
                        annotation.push('\n');
                    }
                    annotation.push_str(&text);
                }
            }
            Event::End(e) => {
                let name = local(e.name().as_ref());
                if name == "author" && in_title_info(&stack) {
                    let full = format!("{first} {last}").trim().to_string();
                    if !full.is_empty() {
                        authors.push(full);
                    }
                    first.clear();
                    last.clear();
                }
                stack.pop();
                if name == "description" {
                    closed = true;
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // Дошли до конца файла, не встретив </description> — файл обрезан или это не fb2.
    if !closed {
        return Err(AppError::bad(
            "Файл fb2 без блока <description>",
            "Broken fb2: no <description> block",
        ));
    }

    meta.author = authors.join(", ");
    if !annotation.is_empty() {
        meta.description = Some(annotation);
    }
    Ok(meta.sanitized())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FB2: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
 <description>
  <title-info>
   <author><first-name>Лев</first-name><last-name>Толстой</last-name></author>
   <author><first-name>Иван</first-name><last-name>Бунин</last-name></author>
   <book-title>Война &amp; мир</book-title>
   <annotation><p>Первая строка.</p><p>Вторая строка.</p></annotation>
   <date>1869</date>
   <lang>ru</lang>
  </title-info>
  <document-info>
   <author><first-name>Не</first-name><last-name>Автор</last-name></author>
  </document-info>
 </description>
 <body><section><p>Текст книги, который парсить не надо.</p></section></body>
</FictionBook>"#;

    #[test]
    fn fb2_title_info_only() {
        let m = parse_fb2(FB2.as_bytes()).unwrap();
        assert_eq!(m.title, "Война & мир");
        assert_eq!(m.author, "Лев Толстой, Иван Бунин"); // document-info не попал
        assert_eq!(m.published.as_deref(), Some("1869"));
        assert_eq!(m.lang.as_deref(), Some("ru"));
        assert_eq!(
            m.description.as_deref(),
            Some("Первая строка.\nВторая строка.")
        );
    }

    #[test]
    fn fb2_without_metadata_still_has_title() {
        let m =
            parse_fb2(br#"<FictionBook><description><title-info/></description></FictionBook>"#)
                .unwrap();
        assert_eq!(m.title, "Untitled");
        assert_eq!(m.author, "");
    }

    /// Серия лежит в атрибуте, а не в тексте, — разбор Event::Text её не видит.
    #[test]
    fn fb2_series_comes_from_the_sequence_attribute() {
        let wrap = |body: &str| {
            format!("<FictionBook><description><title-info><book-title>t</book-title>{body}</title-info></description></FictionBook>")
        };

        // пустой элемент — обычная форма
        let m =
            parse_fb2(wrap(r#"<sequence name="Гарри Поттер" number="2"/>"#).as_bytes()).unwrap();
        assert_eq!(m.series.as_deref(), Some("Гарри Поттер"));

        // парный тег с вложенной подсерией — берём внешнюю
        let m = parse_fb2(
            wrap(r#"<sequence name="Плоский мир"><sequence name="Стража"/></sequence>"#).as_bytes(),
        )
        .unwrap();
        assert_eq!(m.series.as_deref(), Some("Плоский мир"));

        // серии нет вовсе, и пустое имя — это тоже её отсутствие
        assert_eq!(parse_fb2(wrap("").as_bytes()).unwrap().series, None);
        assert_eq!(
            parse_fb2(wrap(r#"<sequence name="  "/>"#).as_bytes())
                .unwrap()
                .series,
            None
        );

        // серия из document-info — про оцифровку, а не про книгу
        let m = parse_fb2(
            r#"<FictionBook><description><title-info><book-title>t</book-title></title-info>
               <document-info><sequence name="Не серия"/></document-info>
               </description></FictionBook>"#
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(m.series, None);
    }

    #[test]
    fn broken_input_is_an_error_not_a_panic() {
        assert!(parse_epub(b"not a zip").is_err());
        assert!(parse_fb2(b"<FictionBook><description>").is_err());
    }

    /// Метаданные пришли из чужого файла — до БД они доезжают только чистыми.
    #[test]
    fn metadata_is_cleaned_before_storing() {
        let fb2 = |title: &str, annotation: &str| {
            format!(
                r#"<FictionBook><description><title-info>
                   <book-title>{title}</book-title>
                   <annotation><p>{annotation}</p></annotation>
                   </title-info></description></FictionBook>"#
            )
        };

        // \0 в text Postgres не кладётся вообще — без чистки это ошибка 500
        let m = parse_fb2(fb2("Вой\u{0}на\u{7} и мир", "ok").as_bytes()).unwrap();
        assert_eq!(m.title, "Война и мир");

        // U+202E переворачивает хвост строки: так "gpj.exe" выглядит как "exe.jpg"
        let m = parse_fb2(fb2("Книга\u{202e}txt.exe", "ok").as_bytes()).unwrap();
        assert_eq!(m.title, "Книгаtxt.exe");

        // длинное поле обрезается, а не уезжает в БД целиком
        let long = "я".repeat(DESCRIPTION_MAX + 500);
        let m = parse_fb2(fb2("t", &long).as_bytes()).unwrap();
        let desc = m.description.unwrap();
        assert_eq!(desc.chars().count(), DESCRIPTION_MAX + 1); // +1 на многоточие
        assert!(desc.ends_with('…'));

        // название из одних управляющих символов — это отсутствие названия
        let m = parse_fb2(fb2("\u{0}\u{1}\u{202e}", "").as_bytes()).unwrap();
        assert_eq!(m.title, "Untitled");
        assert_eq!(m.description, None);
    }

    /// Обрезка идёт по символам: срез по байту посреди utf-8 паниковал бы.
    #[test]
    fn truncation_never_splits_a_character() {
        for len in 0..40 {
            let s = "日本語ёж".repeat(len);
            for limit in [0, 1, 5, 17, 300] {
                let out = clean(&s, limit);
                assert!(out.chars().count() <= limit + 1, "{len} {limit}");
            }
        }
    }
}
