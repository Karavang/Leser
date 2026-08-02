//! Разбор поисковой строки: слова и их транслитерация.
//!
//! Само сравнение делает Postgres (`ilike` + триграммы из pg_trgm), здесь
//! только подготовка запроса — то, для чего SQL неудобен.

/// Латиница → кириллица, длинные сочетания раньше коротких: иначе `sh`
/// разобралось бы как `с`+`х`. Одностороннее: набирают латиницей русское
/// название, обратное («Discworld» кириллицей) не встречается.
const TRANSLIT: &[(&str, &str)] = &[
    ("shch", "щ"),
    ("sch", "щ"),
    ("yo", "ё"),
    ("yu", "ю"),
    ("ya", "я"),
    ("ye", "е"),
    ("zh", "ж"),
    ("kh", "х"),
    ("ts", "ц"),
    ("ch", "ч"),
    ("sh", "ш"),
    ("eh", "э"),
    ("jo", "ё"),
    ("ju", "ю"),
    ("ja", "я"),
    ("a", "а"),
    ("b", "б"),
    ("c", "ц"),
    ("d", "д"),
    ("e", "е"),
    ("f", "ф"),
    ("g", "г"),
    ("h", "х"),
    ("i", "и"),
    ("j", "й"),
    ("k", "к"),
    ("l", "л"),
    ("m", "м"),
    ("n", "н"),
    ("o", "о"),
    ("p", "п"),
    ("q", "к"),
    ("r", "р"),
    ("s", "с"),
    ("t", "т"),
    ("u", "у"),
    ("v", "в"),
    ("w", "в"),
    ("x", "кс"),
    ("y", "ы"),
    ("z", "з"),
];

/// Раскладка ЙЦУКЕН по физическим клавишам: что напечатается, если забыть
/// переключить язык. «rfr pfdjtdsdfnm» — это «как завоевывать» вслепую.
/// Таблица одна на оба направления, промах бывает в обе стороны.
const LAYOUT: &[(char, char)] = &[
    ('q', 'й'),
    ('w', 'ц'),
    ('e', 'у'),
    ('r', 'к'),
    ('t', 'е'),
    ('y', 'н'),
    ('u', 'г'),
    ('i', 'ш'),
    ('o', 'щ'),
    ('p', 'з'),
    ('[', 'х'),
    (']', 'ъ'),
    ('a', 'ф'),
    ('s', 'ы'),
    ('d', 'в'),
    ('f', 'а'),
    ('g', 'п'),
    ('h', 'р'),
    ('j', 'о'),
    ('k', 'л'),
    ('l', 'д'),
    (';', 'ж'),
    ('\'', 'э'),
    ('z', 'я'),
    ('x', 'ч'),
    ('c', 'с'),
    ('v', 'м'),
    ('b', 'и'),
    ('n', 'т'),
    ('m', 'ь'),
    (',', 'б'),
    ('.', 'ю'),
    ('/', '.'),
    ('`', 'ё'),
];

/// Набирали русское, а раскладка стояла английская.
fn from_latin_layout(s: &str) -> String {
    remap(s, |c| LAYOUT.iter().find(|(l, _)| *l == c).map(|(_, r)| *r))
}

/// И наоборот: английское название вслепую по-русски («Рфккн» вместо «Harry»).
fn from_cyrillic_layout(s: &str) -> String {
    remap(s, |c| LAYOUT.iter().find(|(_, r)| *r == c).map(|(l, _)| *l))
}

fn remap(s: &str, f: impl Fn(char) -> Option<char>) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| f(c).unwrap_or(c))
        .collect()
}

/// Переписывает латиницу кириллицей. Нелатинские символы проходят как есть,
/// поэтому на кириллическом запросе это тождественная замена.
fn transliterate(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut rest = lower.as_str();

    'outer: while !rest.is_empty() {
        for (latin, cyr) in TRANSLIT {
            if let Some(tail) = rest.strip_prefix(latin) {
                out.push_str(cyr);
                rest = tail;
                continue 'outer;
            }
        }
        let c = rest.chars().next().expect("rest не пуст");
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `%` и `_` — подстановки LIKE, а не то, что ввёл человек. Экранируем их
/// и сам `\`, чтобы запрос `100%` искал процент, а не «что угодно».
fn escape_like(w: &str) -> String {
    let mut out = String::with_capacity(w.len());
    for c in w.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Прочтения запроса: как набрано, транслитерация и обе раскладки. Книга
/// подходит, если совпал любой набор целиком. Одинаковые прочтения схлопнуты,
/// поэтому у кириллического запроса вариант ровно один.
///
/// Разбиение на слова важнее, чем кажется: поиск одной подстрокой не находил
/// «карнеги друзей», потому что такой строки нет ни в названии, ни в авторе —
/// автор в одном поле, слово из названия в другом.
pub struct Query {
    pub variants: Vec<Vec<String>>,
}

impl Query {
    pub fn parse(raw: &str) -> Self {
        let split = |s: &str| {
            s.split_whitespace()
                .map(escape_like)
                .filter(|w| !w.is_empty())
                .collect::<Vec<_>>()
        };

        let mut variants: Vec<Vec<String>> = Vec::new();
        for reading in [
            raw.to_lowercase(),
            transliterate(raw),
            from_latin_layout(raw),
            from_cyrillic_layout(raw),
        ] {
            let words = split(&reading);
            if !words.is_empty() && !variants.contains(&words) {
                variants.push(words);
            }
        }
        Self { variants }
    }

    pub fn is_empty(&self) -> bool {
        self.variants.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_becomes_cyrillic() {
        assert_eq!(transliterate("kak zavoevat"), "как завоеват");
        assert_eq!(transliterate("Karnegi"), "карнеги");
        assert_eq!(transliterate("Roulingh"), "роулингх");
        // длинные сочетания разбираются раньше коротких
        assert_eq!(transliterate("shchi"), "щи");
        assert_eq!(transliterate("shishki"), "шишки");
        assert_eq!(transliterate("chizh"), "чиж");
        // кириллица проходит насквозь
        assert_eq!(transliterate("Гарри Поттер"), "гарри поттер");
        // цифры и знаки не трогаем
        assert_eq!(transliterate("Kniga 2!"), "книга 2!");
    }

    #[test]
    fn wrong_layout_is_read_back() {
        // «как завоевывать» вслепую с английской раскладкой
        assert_eq!(from_latin_layout("rfr pfdjtdsdfnm"), "как завоевывать");
        assert_eq!(from_latin_layout("Rfhytub"), "карнеги");
        // и наоборот: английское название по-русски
        assert_eq!(from_cyrillic_layout("Рфккн"), "harry");
        // знаки препинания на своих клавишах
        assert_eq!(from_latin_layout(","), "б");
    }

    #[test]
    fn query_splits_into_words() {
        let q = Query::parse("  Карнеги   друзей ");
        assert_eq!(q.variants[0], ["карнеги", "друзей"]);
        assert!(!q.is_empty());
        assert!(Query::parse("").is_empty());
    }

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(Query::parse("100%").variants[0], ["100\\%"]);
        assert_eq!(Query::parse("a_b").variants[0], ["a\\_b"]);
    }

    /// Каждое прочтение попадает в набор ровно один раз: у чисто кириллического
    /// запроса транслитерация ничего не меняет, и лишнего прохода по базе нет.
    #[test]
    fn readings_are_deduplicated() {
        let q = Query::parse("гарри поттер");
        assert_eq!(q.variants.len(), 2); // как набрано + чтение как раскладки

        // латиница даёт и транслитерацию, и раскладку — оба прочтения разные
        let q = Query::parse("kak zavoevat");
        assert!(q.variants.contains(&vec!["как".into(), "завоеват".into()]));
        assert!(q.variants.len() >= 3);

        // цифры одинаковы во всех прочтениях
        assert_eq!(Query::parse("2024").variants.len(), 1);
    }
}
