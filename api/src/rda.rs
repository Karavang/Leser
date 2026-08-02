//! Чтение `.rda` — формата, которым R сохраняет свои данные.
//!
//! На GitHub книги нередко лежат именно так: репозиторий
//! `bradleyboehmke/harrypotter` хранит семь книг как символьные векторы,
//! по элементу на главу. Готового крейта под RData нет, но нужное
//! подмножество формата небольшое: распаковать и достать первый строковый
//! вектор — это и есть текст книги.
//!
//! Наружу отдаётся fb2. Так книга сразу читается имеющейся читалкой,
//! ищется наравне с остальными и не заводит третий формат во всём остальном
//! коде — конвертация один раз на входе дешевле поддержки везде.

use std::io::Read;

use crate::{AppError, Result};

/// Потолок на распакованное: `.rda` жмётся отлично, и безобидный на вид
/// файл может развернуться в гигабайты.
const MAX_UNPACKED: u64 = 128 * 1024 * 1024;

/// Строк в векторе и длина строки. R-вектор на миллион элементов — это уже
/// не книга, а попытка занять память.
const MAX_STRINGS: usize = 100_000;
const MAX_STRING_LEN: usize = 32 * 1024 * 1024;

// Типы объектов R, которые нам встречаются. Остальные не нужны: в книге
// только строки, поэтому на незнакомом типе честно останавливаемся.
const NILVALUE_SXP: u8 = 254;
const NILSXP: u8 = 0;
const SYMSXP: u8 = 1;
const LISTSXP: u8 = 2;
const CHARSXP: u8 = 9;
const STRSXP: u8 = 16;
const VECSXP: u8 = 19;
const REFSXP: u8 = 255;

const HAS_ATTR: u32 = 0x200;
const HAS_TAG: u32 = 0x400;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len());
        let end = end.ok_or_else(|| AppError::bad("Файл .rda обрывается посередине"))?;
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    /// Числа в RData всегда big-endian: заголовок `X` — это XDR.
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// Читает объект и возвращает строковый вектор, если это он. Остальное
    /// разбирается ровно настолько, чтобы дойти до нужного.
    fn value(&mut self) -> Result<Option<Vec<String>>> {
        let flags = self.i32()? as u32;
        let ty = (flags & 0xFF) as u8;

        match ty {
            NILVALUE_SXP | NILSXP => Ok(None),

            // Пара «имя → значение»: так save() хранит переменные.
            // Имя нам не нужно, нужно значение.
            LISTSXP => {
                if flags & HAS_ATTR != 0 {
                    self.value()?;
                }
                if flags & HAS_TAG != 0 {
                    self.value()?;
                }
                let car = self.value()?;
                let cdr = self.value()?;
                Ok(car.or(cdr))
            }

            SYMSXP => {
                self.value()?; // имя символа, дальше не нужно
                Ok(None)
            }

            CHARSXP => {
                let len = self.i32()?;
                if len >= 0 {
                    self.take(len as usize)?;
                }
                Ok(None)
            }

            STRSXP => {
                let n = self.i32()?;
                if !(0..=MAX_STRINGS as i32).contains(&n) {
                    return Err(AppError::bad("Слишком большой вектор в .rda"));
                }
                let mut out = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    out.push(self.charsxp()?);
                }
                if flags & HAS_ATTR != 0 {
                    self.value()?;
                }
                Ok(Some(out))
            }

            // Обычный список: книга может лежать внутри, берём первое найденное.
            VECSXP => {
                let n = self.i32()?;
                if !(0..=MAX_STRINGS as i32).contains(&n) {
                    return Err(AppError::bad("Слишком большой список в .rda"));
                }
                let mut found = None;
                for _ in 0..n {
                    let v = self.value()?;
                    found = found.or(v);
                }
                if flags & HAS_ATTR != 0 {
                    self.value()?;
                }
                Ok(found)
            }

            // Ссылка на уже прочитанный символ. Индекс либо в старших битах
            // флагов, либо отдельным числом — своё значение он не несёт.
            REFSXP => {
                if flags >> 8 == 0 {
                    self.i32()?;
                }
                Ok(None)
            }

            other => Err(AppError::bad(format!(
                "В .rda объект типа {other}, читать такое не умеем"
            ))),
        }
    }

    fn charsxp(&mut self) -> Result<String> {
        let flags = self.i32()? as u32;
        if (flags & 0xFF) as u8 != CHARSXP {
            return Err(AppError::bad("Ожидалась строка в векторе .rda"));
        }
        let len = self.i32()?;
        if len < 0 {
            return Ok(String::new()); // NA
        }
        let len = len as usize;
        if len > MAX_STRING_LEN {
            return Err(AppError::bad("Слишком длинная строка в .rda"));
        }
        // R помечает кодировку в levels, но на практике это utf-8 или latin1;
        // from_utf8_lossy не роняет разбор на втором варианте.
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}

/// Снимает обёртку: R жмёт `.rda` gzip, bzip2 или xz, а бывает и без сжатия.
/// Какой именно — видно по первым байтам, расширение об этом молчит.
fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let limited = |r: &mut dyn Read, out: &mut Vec<u8>| -> Result<()> {
        r.take(MAX_UNPACKED)
            .read_to_end(out)
            .map_err(|e| AppError::bad(format!("Не удалось распаковать .rda: {e}")))?;
        Ok(())
    };

    match bytes {
        [0x1f, 0x8b, ..] => limited(&mut flate2::read::GzDecoder::new(bytes), &mut out)?,
        [b'B', b'Z', b'h', ..] => limited(&mut bzip2::read::BzDecoder::new(bytes), &mut out)?,
        [0xfd, b'7', b'z', b'X', b'Z', ..] => {
            limited(&mut liblzma::read::XzDecoder::new(bytes), &mut out)?
        }
        // несжатый .rda начинается сразу с заголовка
        _ => out = bytes.to_vec(),
    }
    Ok(out)
}

/// Главы книги из `.rda`. Пустые строки отбрасываются: в R-векторах
/// они встречаются как заполнители.
pub fn chapters(bytes: &[u8]) -> Result<Vec<String>> {
    let data = decompress(bytes)?;

    // RDX2/RDX3 — сериализация с заголовком; RDA/RDB — другое, не наше.
    let body = data
        .strip_prefix(b"RDX2\nX\n")
        .or_else(|| data.strip_prefix(b"RDX3\nX\n"))
        .ok_or_else(|| AppError::bad("Это не .rda в формате RDX2/RDX3"))?;

    let mut r = Reader { data: body, pos: 0 };
    r.i32()?; // версия формата
    r.i32()?; // версия писавшего R
    r.i32()?; // минимальная версия читателя
    if body.starts_with(&[0, 0, 0, 3]) {
        // RDX3 добавил строку с родной кодировкой — пропускаем
        let len = r.i32()?;
        if len > 0 {
            r.take(len as usize)?;
        }
    }

    let found = r
        .value()?
        .ok_or_else(|| AppError::bad("В .rda нет текста: строкового вектора не нашлось"))?;

    let chapters: Vec<String> = found
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if chapters.is_empty() {
        return Err(AppError::bad("В .rda нет текста"));
    }
    Ok(chapters)
}

/// Абзацы главы.
///
/// Перевода строки внутри может не быть вовсе: в тех же книгах про Гарри
/// Поттера абзацы разделены парой идеографических пробелов U+3000, и по
/// `\n` вся глава осталась бы одним абзацем на сорок тысяч знаков.
fn paragraphs(chapter: &str) -> Vec<&str> {
    chapter
        .split(['\n', '\r', '\u{3000}'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Собирает из глав fb2 — формат, который здесь уже умеют и читать, и искать.
/// Название берётся из имени файла: внутри `.rda` его нет, там только данные.
pub fn to_fb2(title: &str, chapters: &[String]) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };

    let mut out = String::with_capacity(chapters.iter().map(String::len).sum::<usize>() + 4096);
    out.push_str(
        r#"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
 <description><title-info>
  <book-title>"#,
    );
    out.push_str(&esc(title));
    out.push_str("</book-title>\n  <annotation><p>");
    out.push_str(&format!("Собрано из .rda, глав: {}", chapters.len()));
    out.push_str("</p></annotation>\n </title-info></description>\n <body>\n");

    for (i, chapter) in chapters.iter().enumerate() {
        let paras = paragraphs(chapter);

        // Первый абзац главы обычно её название («THE BOY WHO LIVED»).
        // Если он длинный — это уже текст, и тогда просто нумеруем.
        let titled = paras.first().is_some_and(|p| p.chars().count() <= 120);
        let head = if titled {
            esc(paras[0])
        } else {
            format!("Глава {}", i + 1)
        };

        out.push_str("  <section>\n   <title><p>");
        out.push_str(&head);
        out.push_str("</p></title>\n");

        // заголовок не дублируем в теле главы
        for para in paras.iter().skip(usize::from(titled)) {
            out.push_str("   <p>");
            out.push_str(&esc(para));
            out.push_str("</p>\n");
        }
        out.push_str("  </section>\n");
    }
    out.push_str(" </body>\n</FictionBook>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Собирает несжатый .rda руками: так проверка не тащит за собой
    /// двоичный файл на сотню килобайт.
    fn fake_rda(strings: &[&str]) -> Vec<u8> {
        let mut d = b"RDX2\nX\n".to_vec();
        d.extend(2i32.to_be_bytes()); // версия формата
        d.extend(0x0004_0200i32.to_be_bytes()); // версия R
        d.extend(0x0002_0300i32.to_be_bytes()); // минимальная версия

        // пара «имя → значение» с тегом
        d.extend((LISTSXP as u32 | HAS_TAG).to_be_bytes());
        d.extend((SYMSXP as u32).to_be_bytes()); // тег
        d.extend((CHARSXP as u32).to_be_bytes());
        d.extend(4i32.to_be_bytes());
        d.extend(b"book");

        d.extend((STRSXP as u32).to_be_bytes()); // значение
        d.extend((strings.len() as i32).to_be_bytes());
        for s in strings {
            d.extend((CHARSXP as u32).to_be_bytes());
            d.extend((s.len() as i32).to_be_bytes());
            d.extend(s.as_bytes());
        }
        d.extend((NILVALUE_SXP as u32).to_be_bytes()); // конец списка
        d
    }

    #[test]
    fn reads_chapters_out_of_a_string_vector() {
        let rda = fake_rda(&["THE BOY WHO LIVED\nMr. Dursley", "  ", "VANISHING GLASS"]);
        let ch = chapters(&rda).unwrap();
        // пустая глава отброшена
        assert_eq!(ch.len(), 2);
        assert!(ch[0].starts_with("THE BOY WHO LIVED"));
        assert_eq!(ch[1], "VANISHING GLASS");
    }

    #[test]
    fn gzip_and_bzip2_wrappers_are_unwrapped() {
        use std::io::Write;
        let plain = fake_rda(&["Глава раз", "Глава два"]);

        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&plain).unwrap();
        assert_eq!(chapters(&gz.finish().unwrap()).unwrap().len(), 2);

        let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        bz.write_all(&plain).unwrap();
        assert_eq!(chapters(&bz.finish().unwrap()).unwrap().len(), 2);
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(chapters(b"").is_err());
        assert!(chapters(b"not an rda at all").is_err());
        // заголовок на месте, дальше обрыв
        assert!(chapters(b"RDX2\nX\n\x00\x00\x00\x02").is_err());
    }

    /// fb2 из глав должен пережить разбор нашим же парсером fb2:
    /// иначе книга сохранится, но не откроется.
    #[test]
    fn produced_fb2_parses_back() {
        let fb2 = to_fb2(
            "Philosophers Stone & Co",
            &["THE BOY WHO LIVED\nMr. Dursley был <нормальным>".to_string()],
        );
        let meta = crate::parse::parse_fb2(fb2.as_bytes()).unwrap();
        assert_eq!(meta.title, "Philosophers Stone & Co");
        assert!(fb2.contains("&lt;нормальным&gt;"));
    }

    /// Ровно тот случай, что в репозитории harrypotter: переводов строк нет,
    /// абзацы разделены идеографическими пробелами.
    #[test]
    fn ideographic_spaces_split_paragraphs() {
        let chapter =
            "THE BOY WHO LIVED\u{3000}\u{3000}Mr. Dursley был обычным.\u{3000}\u{3000}Он работал.";
        assert_eq!(
            paragraphs(chapter),
            [
                "THE BOY WHO LIVED",
                "Mr. Dursley был обычным.",
                "Он работал."
            ]
        );

        let fb2 = to_fb2("Philosophers Stone", &[chapter.to_string()]);
        // название главы взято из первого абзаца, а не «Глава 1»
        assert!(fb2.contains("<title><p>THE BOY WHO LIVED</p></title>"));
        // и в теле оно не повторяется
        assert!(!fb2.contains("<p>THE BOY WHO LIVED</p>\n   <p>"));
        assert_eq!(fb2.matches("   <p>").count(), 2);

        // длинный первый абзац — это уже текст, глава просто нумеруется
        let long = "а".repeat(200);
        let fb2 = to_fb2("t", &[long]);
        assert!(fb2.contains("<title><p>Глава 1</p></title>"));
    }
}
