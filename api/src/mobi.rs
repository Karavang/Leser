//! MOBI и AZW3 — снаружи одно и то же: база Palm Database, внутри которой
//! лежит html книги, разрезанный на записи и сжатый.
//!
//! Разбор свой, а не крейтом: работы здесь на один заголовок и один LZ77,
//! зато html дальше идёт через тот же безопасный разбор, что и epub, и
//! картинки достаются из тех же записей.
//!
//! **DRM мы не снимаем.** Файл из магазина Amazon зашифрован, и такая книга
//! честно не открывается — обходить это библиотека не умеет и не будет.

use base64::Engine;

use crate::{
    parse::Meta,
    reader::{decode, Chapter},
    AppError, Result,
};

/// Заголовок Palm Database, дальше идёт список записей по 8 байт.
const PDB: usize = 78;
/// MOBI-заголовок лежит в нулевой записи сразу за 16 байтами PalmDOC.
const MOBI: usize = 16;

fn broken() -> AppError {
    AppError::bad(
        "Файл mobi обрывается на середине",
        "Broken mobi: the file ends mid-record",
    )
}

fn u16at(b: &[u8], at: usize) -> Result<usize> {
    let s = b.get(at..at + 2).ok_or_else(broken)?;
    Ok(u16::from_be_bytes([s[0], s[1]]) as usize)
}

fn u32at(b: &[u8], at: usize) -> Result<usize> {
    let s = b.get(at..at + 4).ok_or_else(broken)?;
    Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

/// Записи файла как срезы: границы — из списка смещений в шапке.
///
/// Смещениям не верим: они приходят из чужого файла, и «запись с 10 по 5»
/// или «за концом файла» тут обычное дело у битой книги.
fn records(b: &[u8]) -> Result<Vec<&[u8]>> {
    let n = u16at(b, 76)?;
    let starts: Vec<usize> = (0..n)
        .map(|i| u32at(b, PDB + i * 8))
        .collect::<Result<_>>()?;

    Ok((0..n)
        .map(|i| {
            let from = starts[i].min(b.len());
            let to = starts
                .get(i + 1)
                .copied()
                .unwrap_or(b.len())
                .clamp(from, b.len());
            &b[from..to]
        })
        .collect())
}

struct Head {
    compression: usize,
    text_records: usize,
    text_length: usize,
    encoding: usize,
    /// номер записи, с которой начинаются картинки
    first_image: usize,
    /// какие служебные хвосты дописаны в конец каждой текстовой записи
    extra_flags: usize,
    /// название книги: смещение и длина внутри нулевой записи
    name: (usize, usize),
    /// где в файле начинается EXTH, если он вообще есть
    exth: Option<usize>,
    /// номер записи с индексом NCX — оглавлением книги
    ncx: usize,
}

fn head(r0: &[u8]) -> Result<Head> {
    let compression = u16at(r0, 0)?;
    let text_length = u32at(r0, 4)?;
    let text_records = u16at(r0, 8)?;

    // Единственный правильный ответ на DRM — сказать, что книга не откроется.
    if u16at(r0, 12)? != 0 {
        return Err(AppError::bad(
            "Книга защищена DRM — такую библиотека не открывает",
            "The book is DRM-protected — the library does not open those",
        ));
    }
    if r0.get(MOBI..MOBI + 4) != Some(b"MOBI".as_slice()) {
        return Err(AppError::bad(
            "Файл mobi без заголовка MOBI",
            "Broken mobi: no MOBI header",
        ));
    }

    let len = u32at(r0, MOBI + 4)?;
    // Заголовок рос со временем: у старых книг поздних полей просто нет,
    // и читать их неоткуда. Отсюда проверки длины, а не просто чтение.
    // Смещения — от подписи «MOBI», то есть на 16 меньше, чем в таблице
    // MobileRead: там они отсчитаны от начала всей нулевой записи.
    let at = |off: usize| {
        if len > off {
            u32at(r0, MOBI + off)
        } else {
            Ok(0)
        }
    };

    Ok(Head {
        compression,
        text_records,
        text_length,
        encoding: at(0x0c)?,
        first_image: at(0x5c)?,
        // Хвосты появились вместе с длинным заголовком; поле u16, а не u32.
        extra_flags: if len >= 0xe4 {
            u16at(r0, MOBI + 0xe2)?
        } else {
            0
        },
        name: (at(0x44)?, at(0x48)?),
        // EXTH идёт сразу за MOBI-заголовком и только если о нём сказано
        exth: (at(0x70)? & 0x40 != 0).then_some(MOBI + len),
        ncx: at(0xe4)?,
    })
}

/// Пункт оглавления: где в тексте начинается и как называется.
struct Entry {
    pos: usize,
    label: String,
}

/// Число переменной длины «вперёд»: по семь бит на байт, старший бит
/// стоит у последнего. Не путать с хвостами записей — там то же, но с конца.
fn varint(d: &[u8], p: &mut usize) -> Option<usize> {
    let mut v = 0usize;
    for _ in 0..5 {
        let c = *d.get(*p)?;
        *p += 1;
        v = (v << 7) | (c & 0x7f) as usize;
        if c & 0x80 != 0 {
            return Some(v);
        }
    }
    None
}

/// Оглавление из индекса NCX. Это то же оглавление, что показывает Kindle:
/// названия и позиции глав, записанные тем, кто книгу собирал. Резать
/// по нему честнее, чем угадывать заголовки по жирному шрифту — в старом
/// mobi тегов заголовков нет вовсе.
///
/// Формат: главная запись INDX с таблицей тегов TAGX, за ней записи
/// с пунктами и записи CNCX со строками названий. У пункта — байт длины
/// имени, имя, контрольный байт и значения тегов подряд, по маске из TAGX.
/// Нужны два тега: 1 — позиция в тексте, 3 — смещение названия в CNCX.
/// Остальные пропускаются по описанию. Битый индекс — `None`: книга тогда
/// режется по заголовкам, как и без индекса. Ничего здесь не паникует:
/// каждое чтение — `get`.
fn ncx(recs: &[&[u8]], h: &Head) -> Option<Vec<Entry>> {
    let main = *recs.get(h.ncx)?;
    if main.get(..4)? != b"INDX" {
        return None;
    }
    let hlen = u32at(main, 4).ok()?;
    let count = u32at(main, 24).ok()?.min(64);
    let ncncx = u32at(main, 52).ok()?.min(64);

    if main.get(hlen..hlen + 4)? != b"TAGX" {
        return None;
    }
    let tagx_len = u32at(main, hlen + 4).ok()?;
    let ctrl = u32at(main, hlen + 8).ok()?;
    // (тег, сколько значений, бит в контрольном байте) — до первой отметки конца
    let mut tags: Vec<(u8, usize, u8)> = Vec::new();
    let mut at = hlen + 12;
    while at + 4 <= hlen + tagx_len {
        let t = main.get(at..at + 4)?;
        if t[3] & 1 != 0 {
            break;
        }
        tags.push((t[0], t[1] as usize, t[2]));
        at += 4;
    }

    let cncx: Vec<u8> = (0..ncncx)
        .filter_map(|i| recs.get(h.ncx + 1 + count + i))
        .flat_map(|r| r.iter().copied())
        .collect();
    let label = |off: usize| -> Option<String> {
        let mut p = off;
        let n = varint(&cncx, &mut p)?;
        Some(
            String::from_utf8_lossy(cncx.get(p..p + n)?)
                .trim()
                .to_string(),
        )
    };

    let mut out = Vec::new();
    for r in 0..count {
        let d = *recs.get(h.ncx + 1 + r)?;
        if d.get(..4)? != b"INDX" {
            return None;
        }
        let idxt = u32at(d, 20).ok()?;
        let n = u32at(d, 24).ok()?.min(4096);
        if d.get(idxt..idxt + 4)? != b"IDXT" {
            return None;
        }
        for i in 0..n {
            let off = u16at(d, idxt + 4 + 2 * i).ok()?;
            let name_len = *d.get(off)? as usize;
            let mut p = off + 1 + name_len;
            let control = *d.get(p)?;
            p += ctrl;
            let (mut pos, mut name) = (None, None);
            for &(tag, values, mask) in &tags {
                if control & mask == 0 {
                    continue;
                }
                for _ in 0..values {
                    let v = varint(d, &mut p)?;
                    match tag {
                        1 => pos = Some(v),
                        3 => name = label(v),
                        _ => {}
                    }
                }
            }
            if let (Some(pos), Some(label)) = (pos, name) {
                out.push(Entry { pos, label });
            }
        }
    }
    Some(out)
}

/// Хвост текстовой записи: за самим текстом могут лежать служебные куски —
/// индекс поиска, границы многобайтового символа. Длина каждого записана
/// в его последних байтах, и её надо отрезать до распаковки.
fn strip_trailing(mut rec: &[u8], flags: usize) -> &[u8] {
    for bit in 1..16 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        let n = trailing_size(rec);
        rec = &rec[..rec.len().saturating_sub(n)];
    }
    // Бит 0 — символ, разрезанный границей записи: сколько его байт попало
    // в конец, написано в младших битах последнего байта.
    if flags & 1 != 0 {
        if let Some(&last) = rec.last() {
            let n = (last & 0x3) as usize + 1;
            rec = &rec[..rec.len().saturating_sub(n)];
        }
    }
    rec
}

/// Размер хвоста: число переменной длины, записанное с конца по семь бит
/// на байт. Старший бит отмечает его начало, поэтому идём вперёд по
/// последним четырём байтам и сбрасываем накопленное на каждой отметке.
fn trailing_size(rec: &[u8]) -> usize {
    let mut n = 0usize;
    for &v in &rec[rec.len().saturating_sub(4)..] {
        if v & 0x80 != 0 {
            n = 0;
        }
        n = (n << 7) | (v & 0x7f) as usize;
    }
    n
}

/// PalmDOC — тот же LZ77: literal-байты, ссылка назад и «пробел плюс буква».
fn unpack(data: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < data.len() {
        let c = data[i];
        i += 1;
        match c {
            0 => out.push(0),
            // столько следующих байт идут как есть
            1..=8 => {
                let n = (c as usize).min(data.len() - i);
                out.extend_from_slice(&data[i..i + n]);
                i += n;
            }
            0x09..=0x7f => out.push(c),
            // пара байт: как далеко назад и сколько оттуда взять
            0x80..=0xbf => {
                let Some(&next) = data.get(i) else { break };
                i += 1;
                let pair = ((c as usize) << 8) | next as usize;
                let dist = (pair >> 3) & 0x07ff;
                let len = (pair & 7) + 3;
                if dist == 0 || dist > out.len() {
                    continue;
                }
                // копируем по байту: куски умеют перекрываться сами с собой
                for _ in 0..len {
                    out.push(out[out.len() - dist]);
                }
            }
            // самый частый случай в английском тексте — пробел и буква
            _ => out.extend_from_slice(&[b' ', c ^ 0x80]),
        }
    }
}

/// Текст книги: записи с первой по `text_records`, распакованные подряд.
fn text(recs: &[&[u8]], h: &Head) -> Result<Vec<u8>> {
    // HUFF/CDIC — второй способ сжатия, со своим словарём и таблицами.
    // ponytail: не реализован. Такие книги почти всегда из магазина, то есть
    // и без того под DRM. Понадобится — распаковщик встаёт сюда же, рядом.
    if !matches!(h.compression, 1 | 2) {
        return Err(AppError::bad(
            "Сжатие HUFF/CDIC библиотека не умеет — сохраните книгу в epub",
            "HUFF/CDIC compression is not supported — save the book as epub",
        ));
    }

    let mut out = Vec::with_capacity(h.text_length.min(64 * 1024 * 1024));
    for rec in recs.iter().skip(1).take(h.text_records) {
        let rec = strip_trailing(rec, h.extra_flags);
        match h.compression {
            1 => out.extend_from_slice(rec),
            _ => unpack(rec, &mut out),
        }
    }
    // Заявленной длине верим только на укорочение: она из того же файла.
    out.truncate(h.text_length.max(1));
    if out.is_empty() {
        return Err(AppError::bad(
            "В книге не нашлось текста",
            "No text found in the book",
        ));
    }
    Ok(out)
}

/// Кодировка текста: в mobi она записана номером кодовой страницы.
fn encoding(code: usize) -> Option<&'static encoding_rs::Encoding> {
    match code {
        65001 => Some(encoding_rs::UTF_8),
        1252 => Some(encoding_rs::WINDOWS_1252),
        1251 => Some(encoding_rs::WINDOWS_1251),
        _ => None,
    }
}

/// Тип картинки по первым байтам: в самом файле он нигде не записан.
fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xff, 0xd8]) {
        Some("image/jpeg")
    } else if b.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && b.starts_with(b"RIFF") && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Картинка по ссылке из разметки. В mobi это `recindex` — номер записи,
/// считая от первой картинки в файле, с единицы.
fn image(src: &str, recs: &[&[u8]], first: usize) -> Option<(String, String)> {
    let n: usize = src.trim().parse().ok()?;
    let rec = *recs.get(first.checked_add(n)?.checked_sub(1)?)?;
    let mime = sniff(rec)?;
    Some((
        mime.to_string(),
        base64::engine::general_purpose::STANDARD.encode(rec),
    ))
}

pub fn chapters(bytes: &[u8]) -> Result<Vec<Chapter>> {
    let recs = records(bytes)?;
    let h = head(recs.first().copied().ok_or_else(broken)?)?;
    let raw = text(&recs, &h)?;
    let enc = encoding(h.encoding);

    let first = h.first_image;
    // 0 и «все единицы» одинаково значат «картинок нет»
    let has_images = first > 0 && first < recs.len();
    let mut image = |src: &str| has_images.then(|| image(src, &recs, first)).flatten();

    let Some(toc) = ncx(&recs, &h).filter(|t| !t.is_empty()) else {
        return crate::reader::single_html(&decode(&raw, enc), &mut image);
    };

    // Пункты из одного номера — подразделы «1», «2», «3» внутри главы,
    // они остаются в тексте своей главы. Книга, где все главы — номера,
    // режется по ним: другого оглавления у неё нет.
    let mut toc: Vec<Entry> = if toc.iter().all(|e| crate::reader::numeric_label(&e.label)) {
        toc
    } else {
        toc.into_iter()
            .filter(|e| !crate::reader::numeric_label(&e.label))
            .collect()
    };
    // индекс записан по уровням, а не по порядку в тексте
    toc.sort_by_key(|e| e.pos);
    toc.dedup_by_key(|e| e.pos);

    let mut pieces = Vec::new();
    let mut from = 0;
    let mut title = String::new();
    for e in toc {
        let mut at = e.pos.min(raw.len());
        // позиция указывает на тег, но на всякий случай не рвём символ utf-8
        while h.encoding == 65001 && at > from && raw.get(at).is_some_and(|b| b & 0xc0 == 0x80) {
            at -= 1;
        }
        if at > from {
            pieces.push((std::mem::take(&mut title), decode(&raw[from..at], enc)));
            from = at;
        }
        title = e.label;
    }
    pieces.push((title, decode(&raw[from..], enc)));
    crate::reader::html_pieces(pieces, &mut image)
}

/// Метаданные из EXTH — списка записей «номер, длина, значение» в конце
/// нулевой записи. Номера — те же, что пишет туда Calibre и Amazon.
pub fn meta(bytes: &[u8]) -> Result<Meta> {
    let recs = records(bytes)?;
    let r0 = recs.first().copied().ok_or_else(broken)?;
    let h = head(r0)?;
    let enc = encoding(h.encoding);
    let text = |b: &[u8]| decode(b, enc);

    let mut meta = Meta {
        // Название лежит отдельно от EXTH, собственным куском нулевой записи.
        title: r0
            .get(h.name.0..h.name.0 + h.name.1)
            .map(&text)
            .unwrap_or_default(),
        ..Meta::default()
    };

    let mut authors: Vec<String> = Vec::new();
    for (kind, value) in exth(r0, h.exth) {
        match kind {
            100 => authors.push(text(value)),
            103 => meta.description = Some(text(value)),
            106 => meta.published = Some(text(value)),
            // Calibre кладёт сюда серию, а 503 — исправленное название,
            // когда в шапке осталось старое.
            503 => meta.title = text(value),
            524 => meta.lang = Some(text(value)),
            // ponytail: серия у Calibre лежит в 517, но в чужих файлах там
            // встречается что угодно. Появится книга серией — проверить и снять.
            _ => {}
        }
    }
    meta.author = authors.join(", ");
    Ok(meta.sanitized())
}

/// Записи EXTH. Битый список — это отсутствие метаданных, а не ошибка:
/// книга от этого читаться не перестаёт.
fn exth(r0: &[u8], at: Option<usize>) -> Vec<(usize, &[u8])> {
    let Some(at) = at else { return Vec::new() };
    if r0.get(at..at + 4) != Some(b"EXTH".as_slice()) {
        return Vec::new();
    }
    let Ok(count) = u32at(r0, at + 8) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut off = at + 12;
    for _ in 0..count.min(1024) {
        let (Ok(kind), Ok(len)) = (u32at(r0, off), u32at(r0, off + 4)) else {
            break;
        };
        // Длина меньше собственного заголовка сдвинула бы нас назад — это
        // бесконечный цикл, а не запись.
        if len < 8 {
            break;
        }
        match r0.get(off + 8..off + len) {
            Some(value) => out.push((kind, value)),
            None => break,
        }
        off += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Собирает настоящий mobi: шапка Palm, PalmDOC+MOBI заголовки, EXTH
    /// и текст, сжатый тем же LZ77. Проверять разбор чужого формата больше
    /// нечем — файла из магазина в тестах взяться неоткуда.
    fn build(text: &str, compressed: bool) -> Vec<u8> {
        build_with(text, compressed, &[])
    }

    /// Число переменной длины «вперёд», как пишет его сам формат.
    fn fwd(mut v: usize) -> Vec<u8> {
        let mut out = vec![(v & 0x7f) as u8 | 0x80];
        v >>= 7;
        while v > 0 {
            out.insert(0, (v & 0x7f) as u8);
            v >>= 7;
        }
        out
    }

    /// Индекс NCX тремя записями: главная с TAGX, пункты с IDXT, строки CNCX.
    fn ncx_records(toc: &[(usize, &str)]) -> Vec<Vec<u8>> {
        let hlen = 56usize;
        let mut main = vec![0u8; hlen];
        main[0..4].copy_from_slice(b"INDX");
        main[4..8].copy_from_slice(&(hlen as u32).to_be_bytes());
        main[24..28].copy_from_slice(&1u32.to_be_bytes()); // одна запись с пунктами
        main[52..56].copy_from_slice(&1u32.to_be_bytes()); // одна запись CNCX
                                                           // теги: 1 — позиция, 3 — название, 4 — глубина, 21 — родитель
        let tags: &[u8] = &[1, 1, 1, 0, 3, 1, 4, 0, 4, 1, 8, 0, 21, 1, 16, 0, 0, 0, 0, 1];
        main.extend_from_slice(b"TAGX");
        main.extend_from_slice(&((12 + tags.len()) as u32).to_be_bytes());
        main.extend_from_slice(&1u32.to_be_bytes()); // один контрольный байт
        main.extend_from_slice(tags);

        let mut cncx = Vec::new();
        let mut entries = vec![0u8; hlen];
        entries[0..4].copy_from_slice(b"INDX");
        entries[4..8].copy_from_slice(&(hlen as u32).to_be_bytes());
        let mut offsets = Vec::new();
        for (i, (pos, label)) in toc.iter().enumerate() {
            offsets.push(entries.len() as u16);
            let name = format!("{i:08}");
            entries.push(name.len() as u8);
            entries.extend_from_slice(name.as_bytes());
            entries.push(1 | 4 | 8); // позиция, название, глубина; родителя нет
            entries.extend(fwd(*pos));
            entries.extend(fwd(cncx.len()));
            cncx.extend(fwd(label.len()));
            cncx.extend_from_slice(label.as_bytes());
            entries.extend(fwd(0));
        }
        let idxt = entries.len();
        entries[20..24].copy_from_slice(&(idxt as u32).to_be_bytes());
        entries[24..28].copy_from_slice(&(toc.len() as u32).to_be_bytes());
        entries.extend_from_slice(b"IDXT");
        for o in offsets {
            entries.extend_from_slice(&o.to_be_bytes());
        }
        vec![main, entries, cncx]
    }

    fn build_with(text: &str, compressed: bool, toc: &[(usize, &str)]) -> Vec<u8> {
        let title = b"Test Book";
        let author = b"Test Author";

        // EXTH: одна запись с автором
        let mut exth = b"EXTH".to_vec();
        exth.extend_from_slice(&0u32.to_be_bytes()); // длину впишем ниже
        exth.extend_from_slice(&1u32.to_be_bytes());
        exth.extend_from_slice(&100u32.to_be_bytes());
        exth.extend_from_slice(&((author.len() + 8) as u32).to_be_bytes());
        exth.extend_from_slice(author);
        let exth_len = exth.len() as u32;
        exth[4..8].copy_from_slice(&exth_len.to_be_bytes());

        // MOBI-заголовок: 0xe8 байт, чтобы дотянуться до всех полей
        let mobi_len = 0xe8usize;
        let mut mobi = vec![0u8; mobi_len];
        mobi[0..4].copy_from_slice(b"MOBI");
        mobi[4..8].copy_from_slice(&(mobi_len as u32).to_be_bytes());
        mobi[0x0c..0x10].copy_from_slice(&65001u32.to_be_bytes());
        let name_off = MOBI + mobi_len + exth.len();
        mobi[0x44..0x48].copy_from_slice(&(name_off as u32).to_be_bytes());
        mobi[0x48..0x4c].copy_from_slice(&(title.len() as u32).to_be_bytes());
        mobi[0x70..0x74].copy_from_slice(&0x40u32.to_be_bytes()); // EXTH есть
        let extra = if toc.is_empty() {
            Vec::new()
        } else {
            ncx_records(toc)
        };
        // индекс — сразу за текстом, запись номер два
        mobi[0xe4..0xe8]
            .copy_from_slice(&(if extra.is_empty() { u32::MAX } else { 2 }).to_be_bytes());

        let body = text.as_bytes();
        let mut packed = Vec::new();
        if compressed {
            // тот же LZ77: literal-байты пачками по восемь
            for chunk in body.chunks(8) {
                packed.push(chunk.len() as u8);
                packed.extend_from_slice(chunk);
            }
        } else {
            packed.extend_from_slice(body);
        }

        let mut r0 = vec![0u8; MOBI];
        r0[0..2].copy_from_slice(&(if compressed { 2u16 } else { 1 }).to_be_bytes());
        r0[4..8].copy_from_slice(&(body.len() as u32).to_be_bytes());
        r0[8..10].copy_from_slice(&1u16.to_be_bytes()); // одна запись текста
        r0.extend_from_slice(&mobi);
        r0.extend_from_slice(&exth);
        r0.extend_from_slice(title);

        let mut records = vec![r0, packed];
        records.extend(extra);
        let mut out = vec![0u8; PDB];
        out[60..64].copy_from_slice(b"BOOK");
        out[64..68].copy_from_slice(b"MOBI");
        out[76..78].copy_from_slice(&(records.len() as u16).to_be_bytes());
        let mut at = PDB + records.len() * 8;
        for (i, r) in records.iter().enumerate() {
            out.extend_from_slice(&(at as u32).to_be_bytes());
            out.extend_from_slice(&(i as u32).to_be_bytes());
            at += r.len();
        }
        for r in &records {
            out.extend_from_slice(r);
        }
        out
    }

    const BOOK: &str = "<html><body><h1>Глава 1</h1><p>Первая.</p>\
                        <h1>Глава 2</h1><p>Вторая.</p></body></html>";

    #[test]
    fn reads_metadata_and_chapters() {
        for compressed in [false, true] {
            let file = build(BOOK, compressed);

            let m = meta(&file).unwrap();
            assert_eq!(m.title, "Test Book", "compressed={compressed}");
            assert_eq!(m.author, "Test Author");

            let ch = chapters(&file).unwrap();
            assert_eq!(ch.len(), 2, "{ch:?}");
            assert_eq!(ch[0].title, "Глава 1");
            assert!(ch[0].html.contains("<p>Первая.</p>"), "{:?}", ch[0]);
            assert_eq!(ch[1].title, "Глава 2");
        }
    }

    /// Старый mobi без тегов заголовков: главы — только в индексе NCX.
    /// Режем по нему; подразделы «1», «2» остаются внутри своей главы,
    /// а то, что до первой главы, приклеивается к ней.
    #[test]
    fn ncx_index_is_the_table_of_contents() {
        let text = "<html><body><p>Титул</p>\
                    <p><b>Глава 1</b></p><p>раз</p>\
                    <p><b>2</b></p><p>два</p>\
                    <p><b>Глава 2</b></p><p>три</p></body></html>";
        let at = |s: &str| text.find(s).unwrap();
        let toc = [
            (at("<p><b>Глава 1"), "Глава 1"),
            (at("<p><b>2"), "2"),
            (at("<p><b>Глава 2"), "Глава 2"),
        ];
        for compressed in [false, true] {
            let ch = crate::reader::chapters("mobi", &build_with(text, compressed, &toc)).unwrap();
            let titles: Vec<&str> = ch.iter().map(|c| c.title.as_str()).collect();
            assert_eq!(titles, ["Глава 1", "Глава 2"], "{ch:?}");
            assert!(ch[0].html.starts_with("<p>Титул</p>"), "{}", ch[0].html);
            assert!(ch[0].html.contains("<p>два</p>"), "{}", ch[0].html);
            assert!(ch[1].html.contains("<p>три</p>"), "{}", ch[1].html);
        }

        // все главы — номера: другого оглавления нет, режем по ним
        // (титул до первой главы здесь ещё отдельный — склейка живёт выше,
        // в reader::chapters)
        let toc = [(at("<p><b>Глава 1"), "1"), (at("<p><b>Глава 2"), "2")];
        let ch = chapters(&build_with(text, true, &toc)).unwrap();
        let titles: Vec<&str> = ch.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["", "1", "2"], "{ch:?}");

        // битый индекс — не ошибка: книга режется по заголовкам, как без него
        let mut file = build_with(text, true, &toc);
        let n = file.len();
        file[n - 40..].fill(0xff);
        assert!(chapters(&file).is_ok());
    }

    /// Сжатие обязано выдержать ссылку назад и «пробел плюс буква» —
    /// в настоящем файле встречается только это.
    #[test]
    fn palmdoc_unpacks_back_references() {
        let mut out = Vec::new();
        // literal "ab", затем ссылка на 2 назад длиной 3 (пишется как 0) → "ababa"
        let pair: u16 = 0x8000 | (2 << 3);
        let mut data = vec![2, b'a', b'b'];
        data.extend_from_slice(&pair.to_be_bytes());
        data.push(b'c' | 0x80); // пробел и буква одним байтом
        unpack(&data, &mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "ababa c");
    }

    /// Книга из магазина зашифрована — её надо честно не открыть,
    /// а не показать читателю мусор.
    #[test]
    fn drm_is_reported_not_bypassed() {
        let mut file = build(BOOK, true);
        let r0 = PDB + 2 * 8;
        file[r0 + 12..r0 + 14].copy_from_slice(&1u16.to_be_bytes());
        let err = chapters(&file).unwrap_err();
        assert!(err.ru.contains("DRM"), "{}", err.ru);
        assert!(err.en.contains("DRM"), "{}", err.en);
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        assert!(chapters(b"").is_err());
        assert!(chapters(b"not a palm database at all").is_err());
        assert!(meta(&[0u8; 200]).is_err());
        // обрубленный на середине файл тоже не должен паниковать
        let file = build(BOOK, true);
        for cut in [80, 100, 150, file.len() - 5] {
            let _ = chapters(&file[..cut]);
        }
    }
}
