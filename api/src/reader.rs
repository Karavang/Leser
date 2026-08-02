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
        _ => Err(AppError::bad("Этот формат читалка не открывает")),
    }?;
    chapters.truncate(MAX_CHAPTERS);
    if chapters.is_empty() {
        return Err(AppError::bad("В книге не нашлось текста"));
    }
    Ok(chapters)
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

        // Цель ссылки может стоять на чём угодно — на секции, на абзаце,
        // на теге, который мы выкидываем. Поэтому ставим её до разбора тега.
        if let Some(id) = attr(e, "id").filter(|_| name != "binary") {
            let n = self.anchors.of(&id);
            self.out.anchor(n);
        }

        match name.as_str() {
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
                if self.section == 0 {
                    self.flush();
                }
                self.section += 1;
                self.opened.push(Open::Through);
            }
            "title" => {
                // название главы — здесь же и заголовок в тексте
                let tag = if self.section <= 1 { "h2" } else { "h3" };
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
                if self.section == 0 {
                    self.flush();
                }
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
        if self.in_title > 0 && self.section <= 1 {
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
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| AppError::bad(format!("Broken fb2: {e}")))?
        {
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

        // <svg><image xlink:href="cover.jpg"/></svg> — обычная обложка в epub
        if name == "img" || name == "image" {
            if let Some(src) = attr(e, "src").or_else(|| attr(e, "href")) {
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
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| AppError::bad(format!("Broken epub: {e}")))?
        {
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
    let mut doc = epub::doc::EpubDoc::from_reader(std::io::Cursor::new(bytes))
        .map_err(|e| AppError::bad(format!("Broken epub: {e}")))?;

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

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(chapters("epub", b"not a zip").is_err());
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
