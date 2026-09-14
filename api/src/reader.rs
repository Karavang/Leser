//! Книга в одном виде для читалки: главы с уже размеченным текстом.
//!
//! Разбор формата стоит на сервере, а не в браузере. Иначе под каждый формат
//! нужна своя читалка (а под epub ещё и распаковка zip в js), и выглядят они
//! по-разному. Здесь epub и fb2 сводятся к одному документу — и читалка одна.
//!
//! **Разметка на выходе безопасна по построению.** Фронт вставляет её через
//! `dangerouslySetInnerHTML`, поэтому ни один тег и ни один атрибут не берётся
//! из файла: теги приходят только из таблиц `fb2_rule`/`html_rule`, атрибуты
//! пишем мы сами, весь текст экранируется, а `<script>`, `<style>` и прочее
//! выкидывается вместе с содержимым. Чужой `onclick=` до выхода не доезжает:
//! атрибуты исходника не читаются вообще, кроме ссылки на картинку.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use base64::Engine;
use quick_xml::events::{BytesStart, BytesText, Event};
use serde::Serialize;

use crate::{AppError, Result};

#[derive(Serialize, Debug, PartialEq)]
pub struct Chapter {
    pub title: String,
    pub html: String,
}

/// Книга целиком уезжает на фронт одним ответом: так читалка знает все главы
/// сразу — оглавление, проценты и переход по главам не требуют новых запросов.
/// «Война и мир» в этой разметке — около 3 МБ.
const HTML_BUDGET: usize = 12 * 1024 * 1024;

/// ponytail: картинки едут data-URI в том же ответе. Отдельная выдача картинки
/// потребовала бы токена прямо в `<img src>`, а его туда не положишь. Потолок
/// держит ответ в разумном размере, лишние картинки просто не поедут.
/// Понадобятся альбомы — эндпоинт с одноразовой ссылкой на картинку.
const IMAGE_BUDGET: usize = 4 * 1024 * 1024;

const MAX_CHAPTERS: usize = 5000;

/// Сущности, которые реально попадаются в книгах. Числовые (`&#8212;`)
/// quick-xml разбирает сам, сюда приходят только именованные.
const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("shy", "\u{ad}"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("hellip", "…"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("bdquo", "„"),
    ("bull", "•"),
    ("middot", "·"),
    ("deg", "°"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("times", "×"),
    ("prime", "′"),
    ("euro", "€"),
    ("pound", "£"),
    ("sect", "§"),
    ("para", "¶"),
    ("dagger", "†"),
];

/// Текст события. Незнакомая сущность не должна ронять разбор целого узла:
/// потерять `&hearts;` не жалко, потерять из-за него абзац — жалко.
fn text_of(t: &BytesText) -> String {
    t.unescape_with(|name| {
        ENTITIES
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(Some(""), |(_, v)| Some(*v))
    })
    .map(|s| s.into_owned())
    // сюда попадает только совсем битое экранирование — текст важнее
    .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).into_owned())
}

/// Имя тега без пространства имён: `l:href` и `href` для нас одно и то же.
fn local(name: &[u8]) -> String {
    String::from_utf8_lossy(name.rsplit(|c| *c == b':').next().unwrap_or(name)).into_owned()
}

fn attr(e: &BytesStart, want: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| local(a.key.as_ref()) == want)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

/// Что делать с тегом исходника.
enum Rule {
    /// выкинуть вместе с содержимым: скрипты, стили, служебные блоки
    Skip,
    /// выкинуть тег, содержимое оставить: `div`, `span`, `a`, незнакомое
    Through,
    /// заменить своим тегом (и, может быть, классом)
    El(&'static str, &'static str),
    /// то же, но закрывать нечего
    Void(&'static str),
}

/// Что мы сделали с открытым элементом — чтобы на закрытии закрыть то же самое.
enum Open {
    Skip,
    Through,
    Tag(&'static str),
}

#[derive(Default)]
struct Out {
    buf: String,
    /// сколько наших элементов открыто. Текст вне них — это отступы между
    /// тегами исходника, а не текст книги.
    depth: usize,
}

impl Out {
    /// Дошли до потолка — дальше не пишем ничего, включая закрывающие теги.
    /// Обрывок разметки браузер закроет сам, а книга такого размера — уже
    /// не книга.
    fn full(&self) -> bool {
        self.buf.len() >= HTML_BUDGET
    }

    fn tag(&mut self, tag: &str, class: &str) {
        if self.full() {
            return;
        }
        self.buf.push('<');
        self.buf.push_str(tag);
        if !class.is_empty() {
            self.buf.push_str(" class=\"");
            self.buf.push_str(class);
            self.buf.push('"');
        }
        self.buf.push('>');
    }

    fn open(&mut self, tag: &str, class: &str) {
        self.tag(tag, class);
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        if !self.full() {
            self.buf.push_str("</");
            self.buf.push_str(tag);
            self.buf.push('>');
        }
        self.depth = self.depth.saturating_sub(1);
    }

    fn text(&mut self, s: &str) {
        if self.full() {
            return;
        }
        // Текст вне абзаца бывает в epub — оборачиваем, иначе он потеряется.
        // Пустой — это перевод строки между тегами, он не нужен.
        if self.depth == 0 {
            if s.trim().is_empty() {
                return;
            }
            self.buf.push_str("<p>");
            self.escape(s);
            self.buf.push_str("</p>");
            return;
        }
        self.escape(s);
    }

    fn escape(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '<' => self.buf.push_str("&lt;"),
                '>' => self.buf.push_str("&gt;"),
                '&' => self.buf.push_str("&amp;"),
                _ => self.buf.push(c),
            }
        }
    }

    /// Заглушка под картинку: байты подставятся вторым проходом, когда
    /// станет известно, нашлись ли они и хватило ли на них потолка.
    fn image(&mut self, i: usize) {
        if !self.full() {
            self.buf.push_str(&placeholder(i));
        }
    }

    /// Цель ссылки. Отдельным пустым элементом, а не атрибутом на соседнем
    /// теге: якорь может стоять на чём угодно, в том числе на теге, который
    /// мы выкидываем, — а так он ставится всегда и одинаково.
    fn anchor(&mut self, n: usize) {
        if !self.full() {
            self.buf.push_str(&format!("<span id=\"a{n}\"></span>"));
        }
    }

    /// Ссылка внутрь книги. Адрес — только наш номер: то, что записано
    /// в файле, в разметку не попадает ни в каком виде.
    fn link(&mut self, n: usize) {
        if !self.full() {
            self.buf.push_str(&format!("<a href=\"#a{n}\">"));
        }
        self.depth += 1;
    }
}

/// Номера целей для ссылок внутри книги. Один на всю книгу: в epub главы
/// ссылаются друг на друга, да и на фронте вся книга лежит одним куском.
#[derive(Default)]
struct Anchors(HashMap<String, usize>);

impl Anchors {
    /// Номер для адреса; одинаковый адрес всегда даёт один и тот же номер,
    /// поэтому ссылка вперёд работает наравне со ссылкой назад.
    fn of(&mut self, key: &str) -> usize {
        let next = self.0.len();
        *self.0.entry(key.to_string()).or_insert(next)
    }
}

/// Начало заглушки. В тексте книги такого не встретится: `<` там экранирован.
const MARK: &str = "<img src=\"img:";

fn placeholder(i: usize) -> String {
    format!("{MARK}{i};\">")
}

/// Подставляет картинки в готовую разметку одним проходом по каждой главе.
/// Чего не нашлось или на что не хватило потолка — исчезает вместе с тегом.
fn put_images(chapters: &mut [Chapter], mut found: impl FnMut(usize) -> Option<(String, String)>) {
    let mut used = 0usize;
    for c in chapters.iter_mut() {
        if !c.html.contains(MARK) {
            continue;
        }
        let mut out = String::with_capacity(c.html.len());
        let mut rest = c.html.as_str();
        while let Some(at) = rest.find(MARK) {
            out.push_str(&rest[..at]);
            rest = &rest[at + MARK.len()..];
            // хвост заглушки: `N;">`
            let (num, tail) = rest.split_once(";\">").unwrap_or((rest, ""));
            rest = tail;
            let img = num
                .parse::<usize>()
                .ok()
                .and_then(&mut found)
                .filter(|(_, b64)| used + b64.len() <= IMAGE_BUDGET);
            if let Some((mime, b64)) = img {
                used += b64.len();
                out.push_str(&format!("<img src=\"data:{mime};base64,{b64}\">"));
            }
        }
        out.push_str(rest);
        c.html = out;
    }
}

pub fn chapters(ext: &str, bytes: &[u8]) -> Result<Vec<Chapter>> {
    let mut chapters = match ext {
        "fb2" => fb2(bytes),
        "epub" => epub(bytes),
        "html" => single_html(&decode(bytes, None), &mut |s| data_image(s)),
        "md" => markdown(bytes),
        "txt" => Ok(split_headings(plain(&decode(bytes, None), false))),
        "pdf" => pdf(bytes),
        "mobi" | "azw3" => crate::mobi::chapters(bytes),
        _ => Err(AppError::bad(
            "Этот формат читалка не открывает",
            "The reader does not open this format",
        )),
    }?;
    chapters.truncate(MAX_CHAPTERS);
    // Пустая глава — это не глава: в оглавлении она строка, ведущая никуда.
    chapters.retain(|c| !c.html.trim().is_empty());
    let chapters = merge_untitled(chapters);
    if chapters.is_empty() {
        return Err(AppError::bad(
            "В книге не нашлось текста",
            "No text found in the book",
        ));
    }
    Ok(chapters)
}

/// Глава без названия среди глав с названиями — не глава, а продолжение
/// предыдущей: эпиграф отдельной секцией и «* * *» в fb2, вторая половина
/// длинной главы в epub. В оглавлении им делать нечего — приклеиваем
/// к предыдущей. Первая без названия (обложка, титул) приклеивается
/// к следующей. Книга совсем без названий не трогается: там нумерация
/// частей и есть оглавление.
///
/// Сноска тоже не глава. В fb2 она отличима по телу `notes`, а конвертеры
/// (Calibre) кладут каждую отдельным файлом с пунктом «1», «2», «3»
/// в оглавлении — поэтому крошечная глава с чисто числовым названием
/// считается сноской и уходит в предыдущую подзаголовком.
fn merge_untitled(chapters: Vec<Chapter>) -> Vec<Chapter> {
    if chapters.iter().all(|c| c.title.is_empty()) {
        return chapters;
    }
    let mut out: Vec<Chapter> = Vec::with_capacity(chapters.len());
    for mut c in chapters {
        if is_note(&c) {
            c.html = c.html.replace("<h2>", "<h3>").replace("</h2>", "</h3>");
            c.title.clear();
        }
        match out.last_mut() {
            Some(prev) if c.title.is_empty() => prev.html.push_str(&c.html),
            _ => out.push(c),
        }
    }
    if out.len() > 1 && out[0].title.is_empty() {
        let cover = out.remove(0);
        out[0].html.insert_str(0, &cover.html);
    }
    out
}

/// ponytail: «1», «[2]», «3.» короче двух килобайт — сноска. Настоящая глава
/// с номером вместо названия такой короткой не бывает.
fn is_note(c: &Chapter) -> bool {
    c.html.len() < 2000 && numeric_label(&c.title)
}

/// Название из одного номера: «1», «[2]», «3.». Так подписывают сноски
/// и подразделы внутри главы, а не главы.
pub(crate) fn numeric_label(t: &str) -> bool {
    let t = t.trim();
    !t.is_empty()
        && t.chars().any(|c| c.is_ascii_digit())
        && t.chars()
            .all(|c| c.is_ascii_digit() || !c.is_alphanumeric())
}

// ── fb2 ──────────────────────────────────────────────────────────────────

fn fb2_rule(name: &str) -> Rule {
    match name {
        "p" => Rule::El("p", ""),
        "v" => Rule::El("p", "v"),
        "subtitle" => Rule::El("h3", ""),
        "emphasis" => Rule::El("em", ""),
        "strong" => Rule::El("strong", ""),
        "strikethrough" => Rule::El("s", ""),
        "sub" => Rule::El("sub", ""),
        "sup" => Rule::El("sup", ""),
        "code" => Rule::El("code", ""),
        "epigraph" | "cite" => Rule::El("blockquote", ""),
        "text-author" => Rule::El("p", "author"),
        "poem" => Rule::El("div", "poem"),
        "stanza" => Rule::El("div", "stanza"),
        "empty-line" => Rule::Void("br"),
        "table" => Rule::El("table", ""),
        "tr" => Rule::El("tr", ""),
        "td" | "th" => Rule::El("td", ""),
        // description целиком: метаданные уже разобраны, в тексте им не место
        "description" => Rule::Skip,
        // в fb2 такому взяться неоткуда — значит, и тексту внутри тоже
        "script" | "style" | "iframe" | "object" | "embed" => Rule::Skip,
        _ => Rule::Through,
    }
}

#[derive(Default)]
struct Fb2 {
    done: Vec<Chapter>,
    out: Out,
    title: String,
    opened: Vec<Open>,
    skip: usize,
    section: usize,
    in_title: usize,
    /// внутри `<body name="notes">` (или comments, footnotes): сноски — одна
    /// глава на всё тело, а не по главе на каждую
    notes: bool,
    /// байты картинок: id из `<binary>` в порядке появления `<image>`
    refs: Vec<String>,
    bins: HashMap<String, (String, String)>,
    /// открытый сейчас `<binary>`: id и тип
    bin: Option<(String, String)>,
    bin_data: String,
    anchors: Anchors,
}

impl Fb2 {
    fn flush(&mut self) {
        let html = std::mem::take(&mut self.out.buf);
        let title = std::mem::take(&mut self.title);
        self.out.depth = 0;
        if !html.trim().is_empty() {
            self.done.push(Chapter { title, html });
        }
    }

    fn start(&mut self, e: &BytesStart) {
        let name = local(e.name().as_ref());

        if self.skip > 0 {
            self.skip += 1;
            self.opened.push(Open::Skip);
            return;
        }

        // Новая глава начинается раньше всего остального. Иначе якорь
        // секции (ниже) лёг бы в буфер до сброса — в хвост предыдущей главы,
        // а из пустого буфера с одним якорем родилась бы глава без текста.
        if name == "section" && self.section == 0 && !self.notes {
            self.flush();
        }

        // Цель ссылки может стоять на чём угодно — на секции, на абзаце,
        // на теге, который мы выкидываем. Поэтому ставим её до разбора тега.
        if let Some(id) = attr(e, "id").filter(|_| name != "binary") {
            let n = self.anchors.of(&id);
            self.out.anchor(n);
        }

        match name.as_str() {
            // Тело — граница главы всегда. Именованное тело — это сноски
            // или комментарии: у него одна глава с названием тела, а секции
            // внутри — подзаголовки, чтобы «1», «2», «3» не стали главами.
            "body" => {
                self.flush();
                self.notes = attr(e, "name").is_some_and(|n| !n.is_empty());
                self.opened.push(Open::Through);
            }
            // Ссылка внутрь книги — оглавление в аннотации, сноска, перекрёстная
            // ссылка. Наружу (`http://`, `mailto:`) не ведём вовсе: адрес в fb2
            // пишет кто угодно, а текст ссылки при этом остаётся на месте.
            "a" => match attr(e, "href").filter(|h| h.starts_with('#')) {
                Some(href) => {
                    let n = self.anchors.of(href.trim_start_matches('#'));
                    self.out.link(n);
                    self.opened.push(Open::Tag("a"));
                }
                None => self.opened.push(Open::Through),
            },
            "binary" => {
                self.bin = Some((
                    attr(e, "id").unwrap_or_default(),
                    attr(e, "content-type").unwrap_or_else(|| "image/jpeg".into()),
                ));
                self.opened.push(Open::Through);
            }
            "section" => {
                self.section += 1;
                self.opened.push(Open::Through);
            }
            "title" => {
                // название главы — здесь же и заголовок в тексте
                let top = self.section == 0 || (self.section == 1 && !self.notes);
                let tag = if top { "h2" } else { "h3" };
                self.in_title += 1;
                self.out.open(tag, "");
                self.opened.push(Open::Tag(tag));
            }
            // абзацы внутри заголовка дали бы <p> внутри <h2> — это невалидно
            // и браузер закрыл бы заголовок раньше времени
            "p" if self.in_title > 0 => self.opened.push(Open::Through),
            "image" => {
                if let Some(href) = attr(e, "href") {
                    self.out.image(self.refs.len());
                    self.refs.push(href.trim_start_matches('#').to_string());
                }
                self.opened.push(Open::Through);
            }
            _ => match fb2_rule(&name) {
                Rule::Skip => {
                    self.skip = 1;
                    self.opened.push(Open::Skip);
                }
                Rule::Through => self.opened.push(Open::Through),
                Rule::Void(tag) => {
                    self.out.tag(tag, "");
                    self.opened.push(Open::Through);
                }
                Rule::El(tag, class) => {
                    self.out.open(tag, class);
                    self.opened.push(Open::Tag(tag));
                }
            },
        }
    }

    fn end(&mut self, name: &str) {
        match self.opened.pop() {
            Some(Open::Skip) => self.skip = self.skip.saturating_sub(1),
            Some(Open::Tag(tag)) => self.out.close(tag),
            _ => {}
        }
        match name {
            "title" => self.in_title = self.in_title.saturating_sub(1),
            "section" => {
                self.section = self.section.saturating_sub(1);
                if self.section == 0 && !self.notes {
                    self.flush();
                }
            }
            "body" => {
                self.flush();
                self.notes = false;
            }
            "binary" => {
                if let Some((id, mime)) = self.bin.take() {
                    let data: String = std::mem::take(&mut self.bin_data)
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    self.bins.insert(id, (mime, data));
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.bin.is_some() {
            self.bin_data.push_str(s);
            return;
        }
        if self.skip > 0 {
            return;
        }
        // Название главы: заголовок секции верхнего уровня, а у сносок —
        // заголовок самого тела («Примечания»).
        let names = if self.notes {
            self.section == 0
        } else {
            self.section <= 1
        };
        if self.in_title > 0 && names {
            let t = s.trim();
            if !t.is_empty() {
                if !self.title.is_empty() {
                    self.title.push(' ');
                }
                self.title.push_str(t);
            }
        }
        self.out.text(s);
    }
}

fn fb2(bytes: &[u8]) -> Result<Vec<Chapter>> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    // Битые книги встречаются: незакрытый тег не должен ронять всю книгу.
    reader.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    let mut s = Fb2::default();

    loop {
        match reader.read_event_into(&mut buf).map_err(|e| {
            AppError::bad(
                format!("Файл fb2 повреждён: {e}"),
                format!("Broken fb2: {e}"),
            )
        })? {
            Event::Start(e) => s.start(&e),
            Event::Empty(e) => {
                let name = local(e.name().as_ref());
                s.start(&e);
                s.end(&name);
            }
            Event::End(e) => s.end(&local(e.name().as_ref())),
            Event::Text(t) => s.text(&text_of(&t)),
            Event::CData(t) => s.text(&String::from_utf8_lossy(&t)),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    s.flush();

    let Fb2 {
        mut done,
        refs,
        bins,
        ..
    } = s;
    put_images(&mut done, |i| {
        refs.get(i).and_then(|id| bins.get(id)).cloned()
    });
    Ok(done)
}

// ── epub ─────────────────────────────────────────────────────────────────

fn html_rule(name: &str) -> Rule {
    match name {
        "script" | "style" | "head" | "title" | "link" | "meta" | "iframe" | "object" | "embed"
        | "video" | "audio" | "form" | "input" | "button" | "select" | "textarea" | "noscript" => {
            Rule::Skip
        }
        "p" | "dd" | "dt" => Rule::El("p", ""),
        "h1" => Rule::El("h2", ""),
        "h2" | "h3" | "h4" | "h5" | "h6" => Rule::El("h3", ""),
        "em" | "i" | "cite" | "var" | "dfn" => Rule::El("em", ""),
        "strong" | "b" => Rule::El("strong", ""),
        "u" | "ins" => Rule::El("u", ""),
        "s" | "strike" | "del" => Rule::El("s", ""),
        "sub" => Rule::El("sub", ""),
        "sup" => Rule::El("sup", ""),
        "code" | "kbd" | "samp" => Rule::El("code", ""),
        "pre" => Rule::El("pre", ""),
        "blockquote" | "q" => Rule::El("blockquote", ""),
        "ul" => Rule::El("ul", ""),
        "ol" => Rule::El("ol", ""),
        "li" => Rule::El("li", ""),
        "table" => Rule::El("table", ""),
        "tr" => Rule::El("tr", ""),
        "td" | "th" => Rule::El("td", ""),
        "br" => Rule::Void("br"),
        "hr" => Rule::Void("hr"),
        _ => Rule::Through,
    }
}

/// Теги, которые в html пишут без закрывающего. В xhtml из epub они приходят
/// самозакрытыми, а в обычном html — нет, и закрывающего можно ждать вечно.
///
/// Без этого один `<meta charset>` в `<head>` уводит весь стек на элемент
/// вперёд: `</head>` закрывает `meta`, `skip` не снимается — и книга целиком
/// оказывается внутри пропущенного `<head>`.
fn void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

struct Xhtml<'a> {
    out: Out,
    title: String,
    opened: Vec<Open>,
    skip: usize,
    /// адреса картинок в порядке появления, ещё не разрешённые в файлы
    srcs: Vec<String>,
    first: usize,
    /// файл этой главы: от него считаются и ссылки, и адреса картинок
    path: PathBuf,
    anchors: &'a mut Anchors,
}

impl Xhtml<'_> {
    fn start(&mut self, e: &BytesStart) {
        let name = local(e.name().as_ref());

        if self.skip > 0 {
            self.skip += 1;
            self.opened.push(Open::Skip);
            return;
        }

        // Цель ссылки — до разбора тега: она может стоять и на теге,
        // который мы выкидываем.
        if let Some(id) = attr(e, "id") {
            let n = self.anchors.of(&anchor_key(&self.path, &id));
            self.out.anchor(n);
        }

        // <svg><image xlink:href="cover.jpg"/></svg> — обычная обложка в epub,
        // recindex — способ mobi сослаться на картинку номером записи
        if name == "img" || name == "image" {
            if let Some(src) = attr(e, "src")
                .or_else(|| attr(e, "href"))
                .or_else(|| attr(e, "recindex"))
            {
                self.out.image(self.srcs.len());
                self.srcs.push(src);
            }
            self.opened.push(Open::Through);
            return;
        }

        // Ссылка внутрь книги: оглавление, сноска, перекрёстная ссылка.
        // Наружу не ведём — двоеточие есть у любой схемы (http:, mailto:,
        // javascript:) и не встречается в относительном адресе файла.
        if name == "a" {
            match attr(e, "href").filter(|h| !h.contains(':') && !h.is_empty()) {
                Some(href) => {
                    let (file, frag) = href.split_once('#').unwrap_or((href.as_str(), ""));
                    let target = if file.is_empty() {
                        self.path.clone()
                    } else {
                        resolve(&self.path, file)
                    };
                    let n = self.anchors.of(&anchor_key(&target, frag));
                    self.out.link(n);
                    self.opened.push(Open::Tag("a"));
                }
                None => self.opened.push(Open::Through),
            }
            return;
        }

        match html_rule(&name) {
            Rule::Skip => {
                self.skip = 1;
                self.opened.push(Open::Skip);
            }
            Rule::Through => self.opened.push(Open::Through),
            Rule::Void(tag) => {
                self.out.tag(tag, "");
                self.opened.push(Open::Through);
            }
            Rule::El(tag, class) => {
                // Первый заголовок в файле — это название главы: и запасное
                // на случай пустого оглавления, и заголовок в тексте. Каким
                // уровнем он записан в книге, неважно: h4 в начале главы —
                // всё равно её название.
                let head = matches!(name.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
                let tag = if head && self.first == 0 && self.title.is_empty() {
                    self.first = self.out.depth + 1;
                    "h2"
                } else {
                    tag
                };
                self.out.open(tag, class);
                self.opened.push(Open::Tag(tag));
            }
        }
    }

    fn end(&mut self, _name: &str) {
        match self.opened.pop() {
            Some(Open::Skip) => self.skip = self.skip.saturating_sub(1),
            Some(Open::Tag(tag)) => {
                self.out.close(tag);
                if self.first > 0 && self.out.depth < self.first {
                    self.first = 0;
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.skip > 0 {
            return;
        }
        if self.first > 0 {
            let t = s.trim();
            if !t.is_empty() {
                if !self.title.is_empty() {
                    self.title.push(' ');
                }
                self.title.push_str(t);
            }
        }
        self.out.text(s);
    }
}

/// Ключ цели ссылки. Пустой якорь — это начало файла, туда ведут ссылки
/// вида `href="chapter3.xhtml"` без решётки.
fn anchor_key(path: &Path, frag: &str) -> String {
    format!("{}#{frag}", path.to_string_lossy())
}

/// Одна страница epub: разметка и, если нашёлся заголовок, название главы.
fn xhtml(
    bytes: &[u8],
    path: &Path,
    img_base: usize,
    anchors: &mut Anchors,
) -> Result<(String, String, Vec<String>)> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    reader.config_mut().check_end_names = false;
    // Кусок mobi, вырезанный по оглавлению, начинается с закрывающих тегов
    // предыдущей главы; в html из чужих рук такое тоже встречается.
    reader.config_mut().allow_unmatched_ends = true;
    let mut buf = Vec::new();
    let mut s = Xhtml {
        out: Out::default(),
        title: String::new(),
        opened: Vec::new(),
        skip: 0,
        srcs: Vec::new(),
        first: 0,
        path: path.to_path_buf(),
        anchors,
    };
    // нумерация картинок сквозная по книге, а разбираем по файлу
    for _ in 0..img_base {
        s.srcs.push(String::new());
    }

    loop {
        match reader.read_event_into(&mut buf).map_err(|e| {
            AppError::bad(
                format!("Файл epub повреждён: {e}"),
                format!("Broken epub: {e}"),
            )
        })? {
            Event::Start(e) => {
                let name = local(e.name().as_ref());
                s.start(&e);
                if void(&name) {
                    s.end(&name);
                }
            }
            Event::Empty(e) => {
                let name = local(e.name().as_ref());
                s.start(&e);
                s.end(&name);
            }
            Event::End(e) => s.end(&local(e.name().as_ref())),
            Event::Text(t) => s.text(&text_of(&t)),
            Event::CData(t) => s.text(&String::from_utf8_lossy(&t)),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let srcs = s.srcs.split_off(img_base);
    Ok((s.out.buf, s.title, srcs))
}

/// Адрес картинки внутри epub: относительный, от папки своей главы,
/// и почти всегда с процентами вместо пробелов и кириллицы.
fn resolve(chapter: &Path, href: &str) -> PathBuf {
    let href = href.split(['#', '?']).next().unwrap_or(href);
    let mut path = chapter.parent().unwrap_or(Path::new("")).to_path_buf();
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                path.pop();
            }
            p => path.push(
                percent_encoding::percent_decode_str(p)
                    .decode_utf8_lossy()
                    .as_ref(),
            ),
        }
    }
    path
}

fn epub(bytes: &[u8]) -> Result<Vec<Chapter>> {
    let mut doc = epub::doc::EpubDoc::from_reader(std::io::Cursor::new(bytes)).map_err(|e| {
        AppError::bad(
            format!("Файл epub повреждён: {e}"),
            format!("Broken epub: {e}"),
        )
    })?;

    // Оглавление ведёт на файлы; сопоставляем по имени файла, чтобы не гадать,
    // от какой папки записан путь в toc и от какой — в списке ресурсов.
    let mut toc: HashMap<String, String> = HashMap::new();
    let mut stack: Vec<&epub::doc::NavPoint> = doc.toc.iter().collect();
    while let Some(p) = stack.pop() {
        if let Some(name) = p
            .content
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        {
            toc.entry(name)
                .or_insert_with(|| p.label.trim().to_string());
        }
        stack.extend(p.children.iter());
    }

    let spine: Vec<String> = doc.spine.iter().map(|s| s.idref.clone()).collect();
    let mut chapters: Vec<Chapter> = Vec::new();
    // путь к файлу картинки в порядке появления, сквозной по всей книге
    let mut srcs: Vec<PathBuf> = Vec::new();
    let mut anchors = Anchors::default();

    for id in spine {
        let Some(res) = doc.resources.get(&id).cloned() else {
            continue;
        };
        if !res.mime.contains("html") {
            continue;
        }
        let Some(data) = doc.get_resource_by_path(&res.path) else {
            continue;
        };

        let (html, heading, hrefs) = xhtml(&data, &res.path, srcs.len(), &mut anchors)?;
        srcs.extend(hrefs.iter().map(|h| resolve(&res.path, h)));

        if html.trim().is_empty() {
            continue;
        }
        let title = res
            .path
            .file_name()
            .and_then(|n| toc.get(n.to_string_lossy().as_ref()))
            .cloned()
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| heading.clone());

        // Заголовок из оглавления в самом тексте может и не стоять — тогда
        // ставим его сами. Если стоит (`heading` не пуст), второй такой же
        // не нужен: в книге он и так уже есть, просто помельче.
        let mut out = Out::default();
        // Ссылка на файл целиком (`href="ch3.xhtml"`, без решётки) ведёт
        // в его начало — значит, начало главы тоже должно быть целью.
        let n = anchors.of(&anchor_key(&res.path, ""));
        out.anchor(n);
        // Заголовок из оглавления в самом тексте может и не стоять — тогда
        // ставим его сами. Если стоит (`heading` не пуст), второй такой же
        // не нужен: в книге он и так уже есть, просто помельче.
        if !title.is_empty() && heading.is_empty() {
            out.open("h2", "");
            out.escape(&title);
            out.close("h2");
        }
        chapters.push(Chapter {
            title,
            html: out.buf + &html,
        });
    }

    let engine = base64::engine::general_purpose::STANDARD;
    put_images(&mut chapters, |i| {
        let path = srcs.get(i)?;
        let mime = doc.get_resource_mime_by_path(path)?;
        let data = doc.get_resource_by_path(path)?;
        Some((mime, engine.encode(data)))
    });
    Ok(chapters)
}

// ── книга одним файлом: html, markdown, txt, pdf, mobi ───────────────────

/// Байты в текст. utf-8, если это utf-8; иначе windows-1251 — в нём лежит
/// половина русских txt и старого html.
///
/// ponytail: BOM и две кодировки, без угадывания по частотам. Промахнуться
/// это может на koi8-r и cp866 — редкость, и лечится ещё одной веткой здесь.
pub fn decode(bytes: &[u8], hint: Option<&'static encoding_rs::Encoding>) -> String {
    // BOM — не гипотеза, а факт, поэтому он важнее и объявленной кодировки.
    if let Some((enc, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return enc
            .decode_without_bom_handling(&bytes[bom..])
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // Валидный utf-8 случайным не бывает, поэтому гипотезу проверяем
        // первой, а объявленную кодировку берём, только когда она не сошлась.
        Err(_) => hint
            .unwrap_or(encoding_rs::WINDOWS_1251)
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

/// Язык книги по её же тексту. Нужен ровно одному — переносам: без `lang`
/// браузер не знает, каким словарём рвать слово, и не рвёт вовсе.
///
/// ponytail: кириллица против латиницы, больше ничего. В txt, pdf и html
/// языку взяться неоткуда, а эти два и покрывают библиотеку; не сошлось —
/// возвращаем `None`, и переносов просто не будет, как и сейчас.
pub fn guess_lang(chapters: &[Chapter]) -> Option<&'static str> {
    let (mut cyr, mut lat) = (0usize, 0usize);
    for c in chapters
        .iter()
        .take(3)
        .flat_map(|c| c.html.chars())
        .take(20_000)
    {
        match c {
            'а'..='я' | 'А'..='Я' | 'ё' | 'Ё' => cyr += 1,
            'a'..='z' | 'A'..='Z' => lat += 1,
            _ => {}
        }
    }
    // Разметка сама по себе латиница, поэтому решает не большинство,
    // а заметный перевес — иначе русская книга оказалась бы английской.
    match () {
        _ if cyr > lat => Some("ru"),
        _ if lat > cyr * 4 && lat > 200 => Some("en"),
        _ => None,
    }
}

/// Разметка одним куском — в главы. Тот же разбор, что у epub: html из
/// чужого файла безопасным не становится оттого, что файл лежит не в zip.
///
/// `image` разрешает адрес картинки в байты — у каждого формата по-своему.
pub(crate) fn single_html(
    text: &str,
    image: &mut dyn FnMut(&str) -> Option<(String, String)>,
) -> Result<Vec<Chapter>> {
    // Объявление кодировки после перекодировки врёт: текст уже utf-8,
    // а quick-xml поверил бы ему и разобрал байты второй раз не тем.
    let text = match text.strip_prefix("<?xml").and_then(|t| t.split_once("?>")) {
        Some((_, rest)) => rest,
        None => text,
    };

    let (html, title, srcs) = xhtml(text.as_bytes(), Path::new(""), 0, &mut Anchors::default())?;

    let mut chapters = split_headings(html);
    if chapters.len() == 1 && chapters[0].title.is_empty() {
        chapters[0].title = title;
    }
    put_images(&mut chapters, |i| srcs.get(i).and_then(|s| image(s)));
    Ok(chapters)
}

/// Книга, уже нарезанная на главы снаружи — по оглавлению mobi. Каждый кусок
/// проходит тот же разбор, что и страница epub; картинки и якоря нумеруются
/// сквозь всю книгу. Кусок без названия берёт его из своего первого
/// заголовка, если он есть.
pub(crate) fn html_pieces(
    pieces: Vec<(String, String)>,
    image: &mut dyn FnMut(&str) -> Option<(String, String)>,
) -> Result<Vec<Chapter>> {
    let mut anchors = Anchors::default();
    let mut srcs: Vec<String> = Vec::new();
    let mut chapters = Vec::new();
    for (title, text) in pieces {
        let (html, heading, more) =
            xhtml(text.as_bytes(), Path::new(""), srcs.len(), &mut anchors)?;
        srcs.extend(more);
        let title = if title.is_empty() { heading } else { title };
        chapters.push(Chapter { title, html });
    }
    put_images(&mut chapters, |i| srcs.get(i).and_then(|s| image(s)));
    Ok(chapters)
}

/// Книга одним файлом приезжает сплошной разметкой. Делим её по заголовкам —
/// по самому верхнему уровню, который встречается в ней больше одного раза.
fn split_headings(html: String) -> Vec<Chapter> {
    let Some(tag) = ["<h2>", "<h3>"]
        .into_iter()
        .find(|t| html.matches(t).count() > 1)
    else {
        // делить не по чему: вся книга — одна глава
        let title = first_heading(&html);
        return vec![Chapter { title, html }];
    };

    let mut out = Vec::new();
    for (i, part) in html.split(tag).enumerate() {
        // всё до первого заголовка — титул, оглавление, предисловие
        if i == 0 {
            if !part.trim().is_empty() {
                out.push(Chapter {
                    title: first_heading(part),
                    html: part.to_string(),
                });
            }
            continue;
        }
        out.push(Chapter {
            title: strip_tags(part.split("</h").next().unwrap_or_default()),
            html: format!("{tag}{part}"),
        });
    }
    out
}

/// Текст первого заголовка в куске разметки — когда названия главы больше
/// взять неоткуда.
fn first_heading(html: &str) -> String {
    ["<h2>", "<h3>"]
        .into_iter()
        .find_map(|tag| {
            let at = html.find(tag)?;
            let rest = &html[at + tag.len()..];
            Some(strip_tags(rest.split("</h").next().unwrap_or_default()))
        })
        .unwrap_or_default()
}

/// Текст заголовка без разметки: в него мог попасть `<em>` или `<br>`.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in html.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    // &amp; последним: иначе «&amp;lt;» развернулось бы в «<»
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

/// Плоский текст (txt, pdf) в разметку. Пустая строка делит абзацы, а внутри
/// абзаца перевод строки — это перенос по ширине исходника, а не новая строка.
///
/// `wrapped` — исходник разбит по ширине страницы (pdf): тогда слово,
/// разорванное дефисом на границе строк, надо собрать обратно. В txt так
/// делать нельзя — там дефис в конце строки это дефис в слове.
fn plain(text: &str, wrapped: bool) -> String {
    let mut out = Out::default();
    let mut para: Vec<&str> = Vec::new();
    // пустая строка в хвосте закрывает последний абзац
    for line in text.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            paragraph(&mut out, &para, wrapped);
            para.clear();
        } else {
            para.push(line);
        }
    }
    out.buf
}

fn paragraph(out: &mut Out, para: &[&str], wrapped: bool) {
    if para.is_empty() {
        return;
    }
    // Заголовок стоит отдельной строкой; строка внутри абзаца заголовком
    // не бывает, как бы она ни выглядела.
    if para.len() == 1 && looks_like_heading(para[0]) {
        out.open("h2", "");
        out.text(para[0].trim());
        out.close("h2");
        return;
    }

    let mut s = String::new();
    for line in para {
        let line = line.trim();
        if wrapped && s.ends_with('-') && s[..s.len() - 1].ends_with(char::is_alphabetic) {
            s.pop();
        } else if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(line);
    }
    out.open("p", "");
    out.text(&s);
    out.close("p");
}

/// Строка, похожая на название главы. В txt и pdf другой разметки нет вовсе,
/// поэтому узнаём только то, в чём трудно ошибиться.
///
/// ponytail: три правила вместо разбора оглавления. Не разделилось — книга
/// остаётся одной главой и читается ровно так же, только без оглавления.
fn looks_like_heading(line: &str) -> bool {
    const WORDS: [&str; 10] = [
        "глава",
        "часть",
        "книга",
        "пролог",
        "эпилог",
        "chapter",
        "part",
        "book",
        "prologue",
        "epilogue",
    ];

    let l = line.trim();
    // Название главы — короткая строка. Длинная — это абзац.
    if l.is_empty() || l.chars().count() > 60 {
        return false;
    }

    let low = l.to_lowercase();
    if let Some(w) = WORDS.iter().find(|w| low.starts_with(**w)) {
        let tail = low[w.len()..].trim_start_matches([' ', '.', ':', '№']);
        // «Глава 7», «Часть II», «Пролог» — да. «Глава семьи молчала» — нет.
        // Не сошлось — это ещё не приговор: «ГЛАВА ВТОРАЯ» узнается ниже.
        if tail.is_empty()
            || tail.starts_with(|c: char| c.is_numeric())
            || tail.chars().all(|c| "ivxlcdm .".contains(c))
        {
            return true;
        }
    }

    // «1. Вступление», «2) Методы» — нумерация разделов, обычная в pdf.
    // Пробел после числа не в счёт: «1941 год был тяжёлым» — не заголовок.
    let after = l.trim_start_matches(|c: char| c.is_ascii_digit());
    if after.len() < l.len() && after.starts_with(['.', ')']) {
        return true;
    }

    // строка целиком заглавными — способ выделить главу, когда больше нечем
    let mut letters = false;
    for c in l.chars() {
        if c.is_alphabetic() {
            letters = true;
            if c.is_lowercase() {
                return false;
            }
        }
    }
    letters
}

/// Картинка, вшитая в сам файл. Только такая и попадает в разметку из html
/// и markdown: за относительным путём нам идти некуда, а по чужой ссылке
/// ходить нельзя — это чужой файл говорит нам, куда сходить.
///
/// Проверяем строго. Тип только растровый: в svg живёт скрипт. Дальше только
/// символы base64 — иначе `data:image/png;base64,"><script>` доехал бы
/// до разметки целиком, кавычкой и всем остальным.
fn data_image(src: &str) -> Option<(String, String)> {
    let (kind, b64) = src.strip_prefix("data:image/")?.split_once(";base64,")?;
    if !matches!(kind, "png" | "jpeg" | "jpg" | "gif" | "webp") {
        return None;
    }
    if b64.is_empty()
        || !b64
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
    {
        return None;
    }
    Some((format!("image/{kind}"), b64.to_string()))
}

/// Markdown. Разбор — pulldown-cmark: списки, вложенность, ``` и ссылки это
/// CommonMark целиком, а не пара строк.
///
/// Его вывод всё равно едет через тот же разбор, что и epub: в markdown можно
/// писать сырой html, и наружу он выходит как есть.
fn markdown(bytes: &[u8]) -> Result<Vec<Chapter>> {
    let text = decode(bytes, None);
    let opts = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_FOOTNOTES;
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new_ext(&text, opts));
    single_html(&html, &mut |s| data_image(s))
}

/// PDF. Страница там свёрстана намертво, поэтому берём из неё текст и верстаем
/// заново — иначе на телефоне книгу можно только масштабировать.
///
/// ponytail: колонтитулы и номера страниц приезжают вместе с текстом
/// отдельными абзацами, картинок нет вовсе. Отличать текст от врезки — это
/// уже про геометрию страницы, и если понадобится, ей место здесь.
fn pdf(bytes: &[u8]) -> Result<Vec<Chapter>> {
    // Разбор шрифтов идёт по чужому файлу и на битом может паниковать;
    // здесь это ошибка формата (400), а не падение задачи (500).
    let text = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes))
        .map_err(|_| AppError::bad("Файл pdf повреждён", "Broken pdf"))?
        .map_err(|e| {
            AppError::bad(
                format!("Файл pdf повреждён: {e}"),
                format!("Broken pdf: {e}"),
            )
        })?;
    Ok(split_headings(plain(&text, true)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(body: &str) -> String {
        format!("<FictionBook><description><title-info><book-title>t</book-title></title-info></description><body>{body}</body></FictionBook>")
    }

    #[test]
    fn fb2_splits_into_chapters_and_keeps_markup() {
        let ch = fb2(
            book(
                "<section><title><p>Глава 1</p></title><p>Текст с <emphasis>наклоном</emphasis>.</p></section>
                 <section><title><p>Глава</p><p>вторая</p></title><p>Ещё текст.</p>
                   <section><title><p>Подглава</p></title><p>Мелочь.</p></section></section>",
            )
            .as_bytes(),
        )
        .unwrap();

        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].title, "Глава 1");
        assert_eq!(
            ch[0].html,
            "<h2>Глава 1</h2><p>Текст с <em>наклоном</em>.</p>"
        );
        // заголовок из нескольких абзацев склеивается, вложенная секция
        // остаётся внутри своей главы, но с заголовком помельче
        assert_eq!(ch[1].title, "Глава вторая");
        assert!(ch[1].html.contains("<h3>Подглава</h3><p>Мелочь.</p>"));
    }

    /// Структура живой книги: заголовок тела, две секции без названия
    /// (вступление и эпиграфы), главы, тело сносок. В оглавлении должны быть
    /// титул, главы и «Примечания» — а не «Часть 2», «1», «2» и пустые главы
    /// между сносками.
    #[test]
    fn fb2_untitled_sections_and_notes_do_not_become_chapters() {
        let ch = chapters(
            "fb2",
            r##"<FictionBook><description/>
            <body>
              <title><p>Автор</p><p>Книга</p></title>
              <section><subtitle>* * *</subtitle><p>Вступление.</p></section>
              <section><epigraph><p>Эпиграф.</p></epigraph></section>
              <section><title><p>Глава первая</p></title><p>Текст<a href="#n1">1</a>.</p></section>
              <section><title><p>Глава вторая</p></title><p>Ещё.</p></section>
            </body>
            <body name="notes">
              <title><p>Примечания</p></title>
              <section id="n1"><title><p>1</p></title><p>Первая сноска.</p></section>
              <section id="n2"><title><p>2</p></title><p>Вторая сноска.</p></section>
            </body></FictionBook>"##
                .as_bytes(),
        )
        .unwrap();

        let titles: Vec<&str> = ch.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Автор Книга", "Глава первая", "Глава вторая", "Примечания"]
        );
        // вступление и эпиграф приклеились к титулу
        assert!(ch[0].html.contains("Вступление."), "{}", ch[0].html);
        assert!(ch[0].html.contains("Эпиграф."), "{}", ch[0].html);
        // сноски — подзаголовками в одной главе, якорь у своей сноски
        let notes = &ch[3].html;
        assert!(notes.contains("<h3>1</h3>"), "{notes}");
        assert!(notes.contains("<h3>2</h3>"), "{notes}");
        assert!(
            notes.contains("<span id=\"a0\"></span><h3>1</h3>"),
            "{notes}"
        );
        assert!(
            ch[1].html.contains("<a href=\"#a0\">1</a>"),
            "{}",
            ch[1].html
        );
    }

    /// Epub после Calibre: обложка без названия, сноски отдельными файлами
    /// с пунктами «1», «2» в оглавлении. Обложка уходит в следующую главу,
    /// сноски — подзаголовками в «Примечания».
    #[test]
    fn cover_and_numbered_notes_are_not_chapters() {
        let ch = |title: &str, html: &str| Chapter {
            title: title.into(),
            html: html.into(),
        };
        let out = merge_untitled(vec![
            ch("", "<img>"),
            ch("Annotation", "<h2>Annotation</h2><p>a</p>"),
            ch("Глава 1", "<h2>Глава 1</h2><p>b</p>"),
            ch("Примечания", "<h2>Примечания</h2>"),
            ch("1", "<h2>1</h2><p>сноска</p>"),
            ch("2", "<h2>2</h2><p>ещё</p>"),
        ]);
        let titles: Vec<&str> = out.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Annotation", "Глава 1", "Примечания"]);
        assert_eq!(out[0].html, "<img><h2>Annotation</h2><p>a</p>");
        assert_eq!(
            out[2].html,
            "<h2>Примечания</h2><h3>1</h3><p>сноска</p><h3>2</h3><p>ещё</p>"
        );
        // роман, где главы названы номерами, не схлопывается
        let big = "<p>x</p>".repeat(500);
        let novel = merge_untitled(vec![ch("1", &big), ch("2", &big)]);
        assert_eq!(novel.len(), 2);
    }

    /// Книга без единого названия остаётся как есть: нумерация частей —
    /// её единственное оглавление.
    #[test]
    fn untitled_only_book_keeps_its_parts() {
        let ch =
            fb2(book("<section><p>Раз.</p></section><section><p>Два.</p></section>").as_bytes())
                .unwrap();
        assert_eq!(ch.len(), 2);
    }

    /// Разметка уезжает на фронт в innerHTML, поэтому из файла в неё не должно
    /// попадать ничего исполняемого.
    #[test]
    fn hostile_markup_is_neutralised() {
        let ch = fb2(book(
            r#"<section><title><p>x</p></title>
                   <script>alert(1)</script>
                   <p onclick="alert(1)" style="x">Текст</p>
                   <a href="javascript:alert(1)">ссылка</a>
                   <p>&lt;script&gt;alert(2)&lt;/script&gt;</p>
                   <p>a &amp; b &lt; c</p></section>"#,
        )
        .as_bytes())
        .unwrap();

        let html = &ch[0].html;
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("onclick"), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(!html.contains("style"), "{html}");
        // текст внутри выкинутого тега уходит вместе с ним, а не вываливается
        assert!(!html.contains("alert(1)"), "{html}");
        // а вот теги, записанные текстом, остаются текстом
        assert!(
            html.contains("&lt;script&gt;alert(2)&lt;/script&gt;"),
            "{html}"
        );
        assert!(html.contains("a &amp; b &lt; c"), "{html}");
        assert!(html.contains("ссылка"), "{html}");
    }

    #[test]
    fn html_from_epub_is_cleaned_the_same_way() {
        let (html, title, srcs) = xhtml(
            r#"<html><head><title>meta</title><style>body{}</style></head>
                <body><h1>Название</h1><p>Текст <b>жирный</b><br/></p>
                <div>Голый текст</div><img src="pic.png"/>
                <p onmouseover="x">хвост</p></body></html>"#
                .as_bytes(),
            Path::new("Text/ch1.xhtml"),
            0,
            &mut Anchors::default(),
        )
        .unwrap();

        assert_eq!(title, "Название");
        assert_eq!(srcs, vec!["pic.png"]);
        assert!(!html.contains("meta") && !html.contains("body{}"), "{html}");
        assert!(!html.contains("onmouseover"), "{html}");
        // <br/> не должен превратиться в </br>: браузер считает его вторым <br>
        assert!(html.contains("<br>") && !html.contains("</br>"), "{html}");
        assert!(html.contains("<h2>Название</h2>"), "{html}");
        assert!(html.contains("<strong>жирный</strong>"), "{html}");
        // текст прямо в div потерялся бы, не заверни мы его в абзац
        assert!(html.contains("<p>Голый текст</p>"), "{html}");
    }

    #[test]
    fn images_become_data_uris_and_missing_ones_disappear() {
        let ch = fb2(book(
            r##"<section><title><p>x</p></title>
                   <p><image l:href="#pic.jpg"/></p>
                   <p><image l:href="#none.jpg"/></p></section>
                   <binary id="pic.jpg" content-type="image/jpeg">QUJD
                   REVG</binary>"##,
        )
        .as_bytes())
        .unwrap();

        let html = &ch[0].html;
        assert!(
            html.contains(r#"<img src="data:image/jpeg;base64,QUJDREVG">"#),
            "{html}"
        );
        // на картинку без байтов ссылки не остаётся вовсе
        assert_eq!(html.matches("<img").count(), 1, "{html}");
        assert!(!html.contains("img:"), "{html}");
    }

    /// Заглушки нумеруются подряд, и `img:1;` не должна подставиться в `img:10;`.
    #[test]
    fn image_placeholders_do_not_overlap() {
        let mut ch = vec![Chapter {
            title: String::new(),
            html: (0..12).map(placeholder).collect::<String>(),
        }];
        put_images(&mut ch, |i| Some(("image/png".into(), format!("N{i}"))));
        for i in 0..12 {
            assert!(ch[0].html.contains(&format!("base64,N{i}\">")), "{i}");
        }
    }

    /// Оглавление в аннотации и сноски — это ссылки внутрь книги, и они
    /// должны остаться рабочими: цель и ссылка получают один номер.
    #[test]
    fn links_inside_the_book_survive() {
        let ch = fb2(book(
            r##"<section id="toc"><title><p>Содержание</p></title>
                    <p><a l:href="#ch2">Глава вторая</a></p>
                    <p><a l:href="http://example.com/x">Наружу</a></p></section>
                    <section id="ch1"><title><p>Первая</p></title>
                    <p>Текст<a l:href="#n1" type="note">1</a></p></section>
                    <section id="ch2"><title><p>Вторая</p></title><p>Ещё</p></section>"##,
        )
        .as_bytes())
        .unwrap();

        let all: String = ch.iter().map(|c| c.html.as_str()).collect();
        // ссылка и её цель — один и тот же номер, хотя цель встретилась позже
        let link = all
            .find("<a href=\"#a")
            .map(|i| &all[i + 11..i + 12])
            .unwrap();
        assert!(
            all.contains(&format!("<span id=\"a{link}\"></span>")),
            "{all}"
        );
        assert!(all.contains(">Глава вторая</a>"), "{all}");
        // сноска ведёт в тело примечаний, поэтому его больше не выкидываем
        assert!(all.contains(">1</a>"), "{all}");
        // наружу ссылки не ведут вовсе, но текст остаётся на месте
        assert!(!all.contains("example.com"), "{all}");
        assert!(all.contains("Наружу"), "{all}");
    }

    /// В epub ссылки идут через файл: `href="ch2.xhtml#top"` из соседней главы
    /// должен попасть в ту же цель, что `id="top"` внутри самой ch2.
    #[test]
    fn epub_links_point_across_files() {
        let mut anchors = Anchors::default();
        let (from, _, _) = xhtml(
            r#"<body><p><a href="../Text/ch2.xhtml#top">вперёд</a>
               <a href="ch2.xhtml">в начало главы</a>
               <a href="https://example.com">наружу</a></p></body>"#
                .as_bytes(),
            Path::new("Text/ch1.xhtml"),
            0,
            &mut anchors,
        )
        .unwrap();
        let (to, _, _) = xhtml(
            r#"<body><p id="top">там</p></body>"#.as_bytes(),
            Path::new("Text/ch2.xhtml"),
            0,
            &mut anchors,
        )
        .unwrap();

        let n = anchors.of("Text/ch2.xhtml#top");
        assert!(from.contains(&format!("<a href=\"#a{n}\">")), "{from}");
        assert!(to.contains(&format!("<span id=\"a{n}\"></span>")), "{to}");
        // ссылка на файл без решётки ведёт в начало главы — это другая цель
        let top = anchors.of("Text/ch2.xhtml#");
        assert!(from.contains(&format!("<a href=\"#a{top}\">")), "{from}");
        assert!(!from.contains("example.com"), "{from}");
        assert!(from.contains("наружу"), "{from}");
    }

    /// В txt нет ничего, кроме пустых строк, — из них и надо собрать книгу.
    #[test]
    fn txt_becomes_paragraphs_and_chapters() {
        let ch = chapters(
            "txt",
            "Глава 1\n\nПервая строка\nи её продолжение.\n\n\nГЛАВА ВТОРАЯ\n\nЕщё текст.\n"
                .as_bytes(),
        )
        .unwrap();

        assert_eq!(ch.len(), 2, "{ch:?}");
        assert_eq!(ch[0].title, "Глава 1");
        // перевод строки внутри абзаца — перенос по ширине, а не новая строка
        assert_eq!(
            ch[0].html,
            "<h2>Глава 1</h2><p>Первая строка и её продолжение.</p>"
        );
        assert_eq!(ch[1].title, "ГЛАВА ВТОРАЯ");
    }

    /// Заголовок в txt узнаётся только по виду, и ошибиться тут легко:
    /// лишняя глава на ровном месте портит оглавление всей книги.
    #[test]
    fn only_real_headings_start_a_chapter() {
        for yes in ["Глава 7", "ЧАСТЬ II", "Пролог", "1. Вступление", "ЭПИЛОГ"]
        {
            assert!(looks_like_heading(yes), "{yes}");
        }
        for no in [
            "Глава семьи молчала весь вечер",
            "1941 год был тяжёлым",
            "обычная строка текста",
            "",
            &"ОЧЕНЬ ДЛИННАЯ СТРОКА ЗАГЛАВНЫМИ, КОТОРАЯ НА САМОМ ДЕЛЕ ЦЕЛЫЙ АБЗАЦ".repeat(2),
        ] {
            assert!(!looks_like_heading(no), "{no}");
        }
    }

    /// Русский txt приезжает в windows-1251 чаще, чем хотелось бы.
    #[test]
    fn cp1251_is_read_as_well_as_utf8() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode("Привет");
        assert_ne!(bytes.as_ref(), "Привет".as_bytes());
        assert_eq!(decode(&bytes, None), "Привет");
        assert_eq!(decode("Привет".as_bytes(), None), "Привет");
        // BOM важнее догадок
        assert_eq!(decode(b"\xef\xbb\xbf\xd0\x9e\xd0\xba", None), "Ок");
    }

    /// В markdown можно писать сырой html — значит, чистить его надо так же,
    /// как всё остальное, а не доверять генератору разметки.
    #[test]
    fn markdown_structure_survives_and_raw_html_does_not() {
        let ch = chapters(
            "md",
            b"# One\n\ntext with *stress*\n\n<script>alert(1)</script>\n\n# Two\n\n- a\n- b\n",
        )
        .unwrap();

        assert_eq!(ch.len(), 2, "{ch:?}");
        assert_eq!(ch[0].title, "One");
        assert!(ch[0].html.contains("<em>stress</em>"), "{:?}", ch[0]);
        assert!(!ch[0].html.contains("alert(1)"), "{:?}", ch[0]);
        assert!(ch[1].html.contains("<li>a</li>"), "{:?}", ch[1]);
    }

    /// Картинка из html и markdown берётся только та, что лежит в самом файле,
    /// и только растровая: в svg живёт скрипт, а по чужой ссылке мы не ходим.
    #[test]
    fn only_inline_raster_images_are_taken() {
        assert_eq!(
            data_image("data:image/png;base64,QUJD"),
            Some(("image/png".into(), "QUJD".into()))
        );
        for bad in [
            "data:image/svg+xml;base64,QUJD",
            "data:text/html;base64,QUJD",
            "data:image/png;base64,\"><script>",
            "data:image/png;base64,",
            "https://example.com/pic.png",
            "pic.png",
        ] {
            assert_eq!(data_image(bad), None, "{bad}");
        }

        let ch = chapters(
            "html",
            br#"<html><body><p><img src="data:image/png;base64,QUJD">
                <img src="https://example.com/x.png"></p></body></html>"#,
        )
        .unwrap();
        let html = &ch[0].html;
        assert!(html.contains("data:image/png;base64,QUJD"), "{html}");
        // за чужой картинкой не ходим — тега не остаётся вовсе
        assert_eq!(html.matches("<img").count(), 1, "{html}");
    }

    /// В обычном html пустые теги пишут без закрывающего — и если ждать
    /// закрывающий, один `<meta>` в шапке съедает всю книгу.
    #[test]
    fn unclosed_void_tags_do_not_swallow_the_book() {
        let ch = chapters(
            "html",
            "<html><head><meta charset=\"utf-8\"><title>t</title></head>
             <body><h2>Заголовок</h2><p>Текст<br>дальше</p></body></html>"
                .as_bytes(),
        )
        .unwrap();

        let html = &ch[0].html;
        assert!(html.contains("<p>Текст<br>дальше</p>"), "{html}");
        assert!(html.contains("<h2>Заголовок</h2>"), "{html}");
        // лишних закрывающих не появилось: стек не уехал
        assert_eq!(html.matches("</p>").count(), 1, "{html}");
    }

    /// Переносы включаются только по языку, а в txt и pdf его негде взять.
    #[test]
    fn language_is_guessed_from_the_text() {
        let of = |html: &str| {
            guess_lang(&[Chapter {
                title: String::new(),
                html: html.into(),
            }])
        };
        assert_eq!(of("<p>Русский текст книги</p>"), Some("ru"));
        // разметка сама по себе латиница — перевесить её она не должна
        assert_eq!(of(&"<p>Текст</p>".repeat(20)), Some("ru"));
        assert_eq!(
            of(&"<p>plain english prose here</p>".repeat(20)),
            Some("en")
        );
        assert_eq!(of("<p>123 — 456</p>"), None);
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(chapters("epub", b"not a zip").is_err());
        assert!(chapters("txt", b"   \n\n  ").is_err()); // текста нет
        assert!(chapters("pdf", b"%PDF-1.4 and then garbage").is_err());
        assert!(chapters("fb2", b"<FictionBook><body></body>").is_err()); // текста нет
        assert!(chapters("djvu", b"whatever").is_err());
    }

    #[test]
    fn relative_image_paths_are_resolved_from_the_chapter() {
        let ch = Path::new("OEBPS/Text/ch1.xhtml");
        assert_eq!(resolve(ch, "pic.png"), Path::new("OEBPS/Text/pic.png"));
        assert_eq!(
            resolve(ch, "../Images/pic%20one.png"),
            Path::new("OEBPS/Images/pic one.png")
        );
        assert_eq!(resolve(ch, "pic.png#frag"), Path::new("OEBPS/Text/pic.png"));
    }
}
