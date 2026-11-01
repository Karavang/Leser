use serde::Serialize;

#[derive(Debug, Default, PartialEq)]
pub struct Tags {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub narrator: Option<String>,
    pub year: Option<String>,
    pub lang: Option<String>,
    pub comment: Option<String>,
    pub duration: Option<f64>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chapter {
    pub title: String,
    pub start: f64,
}

impl Tags {
    fn set(&mut self, key: &str, value: String) {
        let value = value.trim().trim_matches('\0').trim().to_string();
        if value.is_empty() {
            return;
        }
        let slot = match key {
            "title" => &mut self.title,
            "album" => &mut self.album,
            "artist" => &mut self.artist,
            "album_artist" => &mut self.album_artist,
            "narrator" => &mut self.narrator,
            "year" => &mut self.year,
            "lang" => &mut self.lang,
            "comment" => &mut self.comment,
            _ => return,
        };
        slot.get_or_insert(value);
    }

    pub fn author(&self) -> Option<&str> {
        self.album_artist.as_deref().or(self.artist.as_deref())
    }
}

pub enum Kind {
    Id3,
    Mp4,
    Flac,
    Ogg,
    Unknown,
}

pub fn kind(head: &[u8]) -> Kind {
    if head.starts_with(b"ID3") {
        Kind::Id3
    } else if head.get(4..8) == Some(b"ftyp") {
        Kind::Mp4
    } else if head.starts_with(b"fLaC") {
        Kind::Flac
    } else if head.starts_with(b"OggS") {
        Kind::Ogg
    } else {
        Kind::Unknown
    }
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0, |n, x| (n << 8) | u64::from(*x))
}

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0, |n, x| (n << 8) | u64::from(*x))
}

fn synchsafe(b: &[u8]) -> usize {
    b.iter().fold(0, |n, x| (n << 7) | usize::from(*x & 0x7f))
}

// ponytail: tags written as "latin1" are cp1251 in half the Russian audiobooks out there.
// High letters outnumbering ASCII ones means cyrillic; Müller with one umlaut stays latin1.
fn latin1(b: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(b) {
        return s.to_string();
    }
    let high = b.iter().filter(|c| **c >= 0xc0).count();
    let ascii = b.iter().filter(|c| c.is_ascii_alphabetic()).count();
    let enc = if high > ascii {
        encoding_rs::WINDOWS_1251
    } else {
        encoding_rs::WINDOWS_1252
    };
    enc.decode_without_bom_handling(b).0.into_owned()
}

fn utf16(b: &[u8], big: bool) -> String {
    let units: Vec<u16> = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            if big {
                u16::from_be_bytes([p[0], p[1]])
            } else {
                u16::from_le_bytes([p[0], p[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn id3_text(enc: u8, b: &[u8]) -> String {
    match enc {
        1 => match b {
            [0xfe, 0xff, rest @ ..] => utf16(rest, true),
            [0xff, 0xfe, rest @ ..] => utf16(rest, false),
            _ => utf16(b, false),
        },
        2 => utf16(b, true),
        3 => String::from_utf8_lossy(b).into_owned(),
        _ => latin1(b),
    }
}

fn first_value(enc: u8, b: &[u8]) -> String {
    let text = id3_text(enc, b);
    text.split('\0')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn split_terminated(enc: u8, b: &[u8]) -> (&[u8], &[u8]) {
    if enc == 1 || enc == 2 {
        let mut i = 0;
        while i + 1 < b.len() {
            if b[i] == 0 && b[i + 1] == 0 {
                return (&b[..i], &b[i + 2..]);
            }
            i += 2;
        }
        (b, &[])
    } else {
        match b.iter().position(|c| *c == 0) {
            Some(i) => (&b[..i], &b[i + 1..]),
            None => (b, &[]),
        }
    }
}

fn unsync(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        out.push(b[i]);
        if b[i] == 0xff && b.get(i + 1) == Some(&0) {
            i += 1;
        }
        i += 1;
    }
    out
}

pub fn id3v2_len(head: &[u8]) -> Option<usize> {
    if head.len() < 10 || !head.starts_with(b"ID3") {
        return None;
    }
    let footer = if head[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + synchsafe(&head[6..10]) + footer)
}

pub fn id3v2(tag: &[u8]) -> Tags {
    let mut tags = Tags::default();
    let Some(total) = id3v2_len(tag) else {
        return tags;
    };
    let ver = tag[3];
    let flags = tag[5];
    let body = &tag[10..total.min(tag.len())];
    let body = if flags & 0x80 != 0 && ver < 4 {
        unsync(body)
    } else {
        body.to_vec()
    };
    let mut at = 0;
    if flags & 0x40 != 0 && body.len() >= 4 {
        at = if ver >= 4 {
            synchsafe(&body[..4])
        } else {
            4 + be(&body[..4]) as usize
        };
    }
    frames(&body[at.min(body.len())..], ver, &mut tags, true);
    tags
}

fn frames(mut b: &[u8], ver: u8, tags: &mut Tags, top: bool) {
    let (id_len, head_len) = if ver == 2 { (3, 6) } else { (4, 10) };
    while b.len() >= head_len && b[0] != 0 {
        let id = String::from_utf8_lossy(&b[..id_len]).into_owned();
        let size = match ver {
            2 => be(&b[3..6]) as usize,
            3 => be(&b[4..8]) as usize,
            _ => synchsafe(&b[4..8]),
        };
        let fflags = if ver == 2 { 0 } else { b[9] };
        let Some(raw) = b.get(head_len..head_len + size) else {
            break;
        };
        b = &b[head_len + size..];

        let skip = match ver {
            3 => fflags & 0xc0 != 0,
            4 => fflags & 0x0c != 0,
            _ => false,
        };
        if skip || raw.is_empty() {
            continue;
        }
        let mut data = if ver >= 4 && fflags & 0x02 != 0 {
            unsync(raw)
        } else {
            raw.to_vec()
        };
        if ver >= 4 && fflags & 0x01 != 0 && data.len() >= 4 {
            data.drain(..4);
        }
        if ver == 3 && fflags & 0x20 != 0 && !data.is_empty() {
            data.remove(0);
        }
        frame(&id, &data, ver, tags, top);
    }
}

fn frame(id: &str, data: &[u8], ver: u8, tags: &mut Tags, top: bool) {
    let key = match id {
        "TIT2" | "TT2" => "title",
        "TALB" | "TAL" => "album",
        "TPE1" | "TP1" => "artist",
        "TPE2" | "TP2" => "album_artist",
        "TCOM" | "TCM" => "narrator",
        "TYER" | "TYE" | "TDRC" | "TDRL" => "year",
        "TLAN" | "TLA" => "lang",
        "COMM" | "COM" => {
            if data.len() > 4 && tags.comment.is_none() {
                let (_, text) = split_terminated(data[0], &data[4..]);
                tags.set("comment", id3_text(data[0], text));
            }
            return;
        }
        "CHAP" if top => {
            let Some(end) = data.iter().position(|c| *c == 0) else {
                return;
            };
            let Some(times) = data.get(end + 1..end + 17) else {
                return;
            };
            let start = be(&times[..4]) as f64 / 1000.0;
            let mut sub = Tags::default();
            frames(&data[end + 17..], ver, &mut sub, false);
            tags.chapters.push(Chapter {
                title: sub.title.unwrap_or_default(),
                start,
            });
            return;
        }
        _ => return,
    };
    if let Some((enc, text)) = data.split_first() {
        tags.set(key, first_value(*enc, text));
    }
}

pub fn id3v1(tail: &[u8]) -> Tags {
    let mut tags = Tags::default();
    if tail.len() < 128 {
        return tags;
    }
    let t = &tail[tail.len() - 128..];
    if !t.starts_with(b"TAG") {
        return tags;
    }
    let field = |r: std::ops::Range<usize>| {
        let raw = &t[r];
        let end = raw.iter().position(|c| *c == 0).unwrap_or(raw.len());
        latin1(&raw[..end])
    };
    tags.set("title", field(3..33));
    tags.set("artist", field(33..63));
    tags.set("album", field(63..93));
    tags.set("year", field(93..97));
    tags
}

pub struct Atom<'a> {
    pub kind: [u8; 4],
    pub body: &'a [u8],
}

pub fn atoms(mut b: &[u8]) -> Vec<Atom<'_>> {
    let mut out = Vec::new();
    while b.len() >= 8 {
        let mut size = be(&b[..4]) as usize;
        let kind = [b[4], b[5], b[6], b[7]];
        let mut head = 8;
        if size == 1 {
            let Some(large) = b.get(8..16) else { break };
            size = be(large) as usize;
            head = 16;
        } else if size == 0 {
            size = b.len();
        }
        if size < head || size > b.len() {
            break;
        }
        out.push(Atom {
            kind,
            body: &b[head..size],
        });
        b = &b[size..];
    }
    out
}

fn child<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    atoms(b)
        .into_iter()
        .find(|a| &a.kind == kind)
        .map(|a| a.body)
}

fn path<'a>(mut b: &'a [u8], kinds: &[&[u8; 4]]) -> Option<&'a [u8]> {
    for kind in kinds {
        b = child(b, kind)?;
    }
    Some(b)
}

pub struct TopAtom {
    pub kind: [u8; 4],
    pub size: u64,
    pub head: u64,
}

pub fn top_atom(head: &[u8], at: u64, file_len: u64) -> Option<TopAtom> {
    if head.len() < 8 {
        return None;
    }
    let kind = [head[4], head[5], head[6], head[7]];
    let (size, hsize) = match be(&head[..4]) {
        1 => (be(head.get(8..16)?), 16),
        0 => (file_len - at, 8),
        n => (n, 8),
    };
    (size >= hsize).then_some(TopAtom {
        kind,
        size,
        head: hsize,
    })
}

pub struct TextTrack {
    pub samples: Vec<(u64, u32, f64)>,
}

pub fn mp4(moov: &[u8]) -> (Tags, Option<TextTrack>) {
    let mut tags = Tags::default();

    if let Some(mvhd) = child(moov, b"mvhd") {
        let (scale, dur) = if mvhd.first() == Some(&1) && mvhd.len() >= 32 {
            (be(&mvhd[20..24]), be(&mvhd[24..32]))
        } else if mvhd.len() >= 20 {
            (be(&mvhd[12..16]), be(&mvhd[16..20]))
        } else {
            (0, 0)
        };
        if scale > 0 && dur > 0 {
            tags.duration = Some(dur as f64 / scale as f64);
        }
    }

    if let Some(udta) = child(moov, b"udta") {
        if let Some(meta) = child(udta, b"meta") {
            let meta = if meta.get(4..8) == Some(b"hdlr") || meta.len() < 4 {
                meta
            } else {
                &meta[4..]
            };
            if let Some(ilst) = child(meta, b"ilst") {
                for item in atoms(ilst) {
                    let key = match &item.kind {
                        b"\xa9nam" => "title",
                        b"\xa9alb" => "album",
                        b"\xa9ART" => "artist",
                        b"aART" => "album_artist",
                        b"\xa9nrt" | b"\xa9wrt" => "narrator",
                        b"\xa9day" => "year",
                        b"ldes" | b"desc" | b"\xa9cmt" => "comment",
                        _ => continue,
                    };
                    if let Some(data) = child(item.body, b"data") {
                        if data.len() > 8 && be(&data[..4]) == 1 {
                            tags.set(key, String::from_utf8_lossy(&data[8..]).into_owned());
                        }
                    }
                }
            }
        }
        if let Some(chpl) = child(udta, b"chpl") {
            tags.chapters = nero_chapters(chpl);
        }
    }

    let traks: Vec<&[u8]> = atoms(moov)
        .into_iter()
        .filter(|a| &a.kind == b"trak")
        .map(|a| a.body)
        .collect();
    let wanted: Vec<u64> = traks
        .iter()
        .filter_map(|t| path(t, &[b"tref", b"chap"]))
        .flat_map(|c| {
            c.as_chunks::<4>()
                .0
                .iter()
                .map(|n| be(n))
                .collect::<Vec<_>>()
        })
        .collect();
    let text = traks.iter().find(|t| {
        path(t, &[b"tkhd"])
            .map(track_id)
            .is_some_and(|id| wanted.contains(&id))
    });
    (tags, text.and_then(|t| text_track(t)))
}

fn track_id(tkhd: &[u8]) -> u64 {
    if tkhd.first() == Some(&1) {
        tkhd.get(20..24).map(be).unwrap_or(0)
    } else {
        tkhd.get(12..16).map(be).unwrap_or(0)
    }
}

fn nero_chapters(chpl: &[u8]) -> Vec<Chapter> {
    let Some(&version) = chpl.first() else {
        return Vec::new();
    };
    let mut at = if version == 1 { 8 } else { 4 };
    let Some(&count) = chpl.get(at) else {
        return Vec::new();
    };
    at += 1;
    let mut out = Vec::new();
    for _ in 0..count {
        let Some(start) = chpl.get(at..at + 8).map(be) else {
            break;
        };
        let Some(&len) = chpl.get(at + 8) else { break };
        let Some(title) = chpl.get(at + 9..at + 9 + usize::from(len)) else {
            break;
        };
        out.push(Chapter {
            title: String::from_utf8_lossy(title).trim().to_string(),
            start: start as f64 / 10_000_000.0,
        });
        at += 9 + usize::from(len);
    }
    out
}

fn text_track(trak: &[u8]) -> Option<TextTrack> {
    let mdia = child(trak, b"mdia")?;
    let mdhd = child(mdia, b"mdhd")?;
    let scale = if mdhd.first() == Some(&1) {
        be(mdhd.get(20..24)?)
    } else {
        be(mdhd.get(12..16)?)
    };
    if scale == 0 {
        return None;
    }
    let stbl = path(mdia, &[b"minf", b"stbl"])?;

    let mut times = Vec::new();
    let mut t = 0u64;
    let stts = child(stbl, b"stts")?;
    for e in stts.get(8..)?.as_chunks::<8>().0 {
        for _ in 0..be(&e[..4]).min(10_000) {
            times.push(t as f64 / scale as f64);
            t += be(&e[4..8]);
        }
    }

    let stsz = child(stbl, b"stsz")?;
    let fixed = be(stsz.get(4..8)?) as u32;
    let count = be(stsz.get(8..12)?) as usize;
    let sizes: Vec<u32> = if fixed > 0 {
        vec![fixed; count.min(10_000)]
    } else {
        stsz.get(12..)?
            .as_chunks::<4>()
            .0
            .iter()
            .take(count)
            .map(|c| be(c) as u32)
            .collect()
    };

    let offsets: Vec<u64> = if let Some(stco) = child(stbl, b"stco") {
        stco.get(8..)?
            .as_chunks::<4>()
            .0
            .iter()
            .map(|n| be(n))
            .collect()
    } else {
        child(stbl, b"co64")?
            .get(8..)?
            .as_chunks::<8>()
            .0
            .iter()
            .map(|n| be(n))
            .collect()
    };
    let stsc: Vec<(usize, usize)> = child(stbl, b"stsc")?
        .get(8..)?
        .as_chunks::<12>()
        .0
        .iter()
        .map(|e| (be(&e[..4]) as usize, be(&e[4..8]) as usize))
        .collect();

    let mut samples = Vec::new();
    let mut n = 0;
    for (i, &base) in offsets.iter().enumerate() {
        let chunk = i + 1;
        let per = stsc
            .iter()
            .rev()
            .find(|(first, _)| *first <= chunk)
            .map(|(_, per)| *per)
            .unwrap_or(1);
        let mut at = base;
        for _ in 0..per {
            let (Some(&size), Some(&start)) = (sizes.get(n), times.get(n)) else {
                break;
            };
            samples.push((at, size, start));
            at += u64::from(size);
            n += 1;
        }
    }
    Some(TextTrack { samples })
}

pub fn text_sample(b: &[u8]) -> String {
    let Some(len) = b.get(..2).map(be) else {
        return String::new();
    };
    let text = b.get(2..2 + len as usize).unwrap_or(&b[2..]);
    match text {
        [0xfe, 0xff, rest @ ..] => utf16(rest, true),
        [0xff, 0xfe, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(text).into_owned(),
    }
    .trim()
    .to_string()
}

pub fn flac(b: &[u8]) -> Tags {
    let mut tags = Tags::default();
    let mut at = 4;
    while let Some(head) = b.get(at..at + 4) {
        let last = head[0] & 0x80 != 0;
        let kind = head[0] & 0x7f;
        let len = be(&head[1..4]) as usize;
        let Some(block) = b.get(at + 4..at + 4 + len) else {
            break;
        };
        match kind {
            0 if block.len() >= 18 => {
                let rate = be(&block[10..13]) >> 4;
                let samples = be(&block[13..18]) & 0xf_ffff_ffff;
                if rate > 0 && samples > 0 {
                    tags.duration = Some(samples as f64 / rate as f64);
                }
            }
            4 => vorbis_comment(block, &mut tags),
            _ => {}
        }
        if last {
            break;
        }
        at += 4 + len;
    }
    tags
}

pub fn ogg(b: &[u8]) -> Tags {
    let mut tags = Tags::default();
    let mut packets: Vec<Vec<u8>> = vec![Vec::new()];
    let mut at = 0;
    while packets.len() <= 2 {
        let Some(head) = b.get(at..at + 27).filter(|h| h.starts_with(b"OggS")) else {
            return tags;
        };
        let segs = usize::from(head[26]);
        let Some(table) = b.get(at + 27..at + 27 + segs) else {
            return tags;
        };
        let mut pos = at + 27 + segs;
        for &len in table {
            let Some(seg) = b.get(pos..pos + usize::from(len)) else {
                return tags;
            };
            packets.last_mut().unwrap().extend_from_slice(seg);
            pos += usize::from(len);
            if len < 255 {
                packets.push(Vec::new());
            }
        }
        at = pos;
    }
    let comments = &packets[1];
    if let Some(body) = comments
        .strip_prefix(b"OpusTags")
        .or_else(|| comments.strip_prefix(b"\x03vorbis"))
    {
        vorbis_comment(body, &mut tags);
    }
    tags
}

fn vorbis_comment(b: &[u8], tags: &mut Tags) {
    let Some(vendor) = b.get(..4).map(le) else {
        return;
    };
    let mut at = 4 + vendor as usize;
    let Some(count) = b.get(at..at + 4).map(le) else {
        return;
    };
    at += 4;
    let mut marks: std::collections::BTreeMap<String, (Option<f64>, Option<String>)> =
        Default::default();
    for _ in 0..count.min(10_000) {
        let Some(len) = b.get(at..at + 4).map(le) else {
            break;
        };
        let Some(raw) = b.get(at + 4..at + 4 + len as usize) else {
            break;
        };
        at += 4 + len as usize;
        let entry = String::from_utf8_lossy(raw);
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        let key = key.to_ascii_uppercase();
        if let Some(n) = key.strip_prefix("CHAPTER") {
            let (num, name) = match n.strip_suffix("NAME") {
                Some(num) => (num.to_string(), true),
                None => (n.to_string(), false),
            };
            let mark = marks.entry(num).or_default();
            if name {
                mark.1 = Some(value.to_string());
            } else {
                mark.0 = clock(value);
            }
            continue;
        }
        let slot = match key.as_str() {
            "TITLE" => "title",
            "ALBUM" => "album",
            "ARTIST" => "artist",
            "ALBUMARTIST" | "ALBUM ARTIST" => "album_artist",
            "PERFORMER" | "COMPOSER" => "narrator",
            "DATE" | "YEAR" => "year",
            "LANGUAGE" => "lang",
            "DESCRIPTION" | "COMMENT" => "comment",
            _ => continue,
        };
        tags.set(slot, value.to_string());
    }
    tags.chapters = marks
        .into_values()
        .filter_map(|(start, title)| {
            Some(Chapter {
                start: start?,
                title: title.unwrap_or_default(),
            })
        })
        .collect();
}

fn clock(v: &str) -> Option<f64> {
    v.trim().split(':').try_fold(0.0, |acc, part| {
        Some(acc * 60.0 + part.parse::<f64>().ok()?)
    })
}

pub fn tidy(mut chapters: Vec<Chapter>, seconds: f64) -> Vec<Chapter> {
    chapters
        .retain(|c| c.start.is_finite() && c.start >= 0.0 && (seconds <= 0.0 || c.start < seconds));
    chapters.sort_by(|a, b| a.start.total_cmp(&b.start));
    chapters.dedup_by(|b, a| (b.start - a.start).abs() < 0.5);
    for c in &mut chapters {
        c.title = crate::parse::clean(&c.title, 200);
    }
    if chapters.len() < 2 {
        return Vec::new();
    }
    chapters.truncate(2000);
    chapters
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn id3_frame(id: &str, body: &[u8]) -> Vec<u8> {
        let mut f = id.as_bytes().to_vec();
        f.extend_from_slice(&(body.len() as u32).to_be_bytes());
        f.extend_from_slice(&[0, 0]);
        f.extend_from_slice(body);
        f
    }

    pub fn id3_tag(frames: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = frames.concat();
        let n = body.len();
        let mut t = b"ID3\x03\x00\x00".to_vec();
        t.extend_from_slice(&[
            ((n >> 21) & 0x7f) as u8,
            ((n >> 14) & 0x7f) as u8,
            ((n >> 7) & 0x7f) as u8,
            (n & 0x7f) as u8,
        ]);
        t.extend_from_slice(&body);
        t
    }

    fn text(enc: u8, s: &[u8]) -> Vec<u8> {
        [&[enc][..], s].concat()
    }

    #[test]
    fn id3v2_reads_book_fields_in_any_encoding_and_chapters() {
        let utf16: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain("Мастер".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode("Булгаков");
        let chap = |id: &str, ms: u32, title: &str| {
            let mut b = id.as_bytes().to_vec();
            b.push(0);
            b.extend_from_slice(&ms.to_be_bytes());
            b.extend_from_slice(&(ms + 1000).to_be_bytes());
            b.extend_from_slice(&[0xff; 8]);
            b.extend(id3_frame("TIT2", &text(3, title.as_bytes())));
            id3_frame("CHAP", &b)
        };
        let tag = id3_tag(&[
            id3_frame("TIT2", &text(1, &utf16)),
            id3_frame("TPE1", &text(0, &cp1251)),
            id3_frame("TALB", &text(3, "Мастер и Маргарита\0второе".as_bytes())),
            id3_frame("TCOM", &text(0, b"M\xfcller")),
            id3_frame("COMM", &text(3, b"eng\0Read by someone")),
            chap("ch0", 0, "Глава 1"),
            chap("ch1", 61_500, "Глава 2"),
        ]);

        let tags = id3v2(&tag);
        assert_eq!(tags.title.as_deref(), Some("Мастер"));
        assert_eq!(tags.artist.as_deref(), Some("Булгаков"));
        assert_eq!(tags.album.as_deref(), Some("Мастер и Маргарита"));
        assert_eq!(tags.narrator.as_deref(), Some("Müller"));
        assert_eq!(tags.comment.as_deref(), Some("Read by someone"));
        assert_eq!(
            tags.chapters,
            vec![
                Chapter {
                    title: "Глава 1".into(),
                    start: 0.0
                },
                Chapter {
                    title: "Глава 2".into(),
                    start: 61.5
                },
            ]
        );
        assert_eq!(id3v2_len(&tag), Some(tag.len()));
    }

    #[test]
    fn id3v1_is_the_fallback_at_the_end_of_the_file() {
        let mut t = b"TAG".to_vec();
        for (s, n) in [
            ("Dracula", 30),
            ("Bram Stoker", 30),
            ("Dracula", 30),
            ("1897", 4),
            ("", 31),
        ] {
            let mut f = s.as_bytes().to_vec();
            f.resize(n, 0);
            t.extend(f);
        }
        let mut file = vec![0u8; 500];
        file.extend(t);
        let tags = id3v1(&file);
        assert_eq!(tags.title.as_deref(), Some("Dracula"));
        assert_eq!(tags.artist.as_deref(), Some("Bram Stoker"));
        assert_eq!(tags.year.as_deref(), Some("1897"));
    }

    fn atom(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let mut a = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        a.extend_from_slice(kind);
        a.extend_from_slice(body);
        a
    }

    fn full(kind: &[u8], body: &[u8]) -> Vec<u8> {
        atom(kind, &[&[0, 0, 0, 0][..], body].concat())
    }

    fn item(kind: &[u8], value: &str) -> Vec<u8> {
        let data = [&1u32.to_be_bytes()[..], &[0; 4], value.as_bytes()].concat();
        atom(kind, &atom(b"data", &data))
    }

    #[test]
    fn mp4_reads_ilst_duration_and_both_kinds_of_chapters() {
        let mut mvhd = vec![0u8; 100];
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&3_725_500u32.to_be_bytes());

        let mut chpl = vec![1, 0, 0, 0, 0, 0, 0, 0, 2];
        for (t, s) in [(0u64, "Intro"), (600_000_000u64, "Part one")] {
            chpl.extend_from_slice(&t.to_be_bytes());
            chpl.push(s.len() as u8);
            chpl.extend_from_slice(s.as_bytes());
        }

        let ilst = [
            item(b"\xa9nam", "Part 1"),
            item(b"\xa9alb", "Dune"),
            item(b"\xa9ART", "Frank Herbert"),
            item(b"\xa9nrt", "Scott Brick"),
        ]
        .concat();
        let meta = full(
            b"meta",
            &[atom(b"hdlr", &[0; 25]), atom(b"ilst", &ilst)].concat(),
        );
        let udta = atom(b"udta", &[meta, atom(b"chpl", &chpl)].concat());

        let mut tkhd_audio = vec![0u8; 84];
        tkhd_audio[12..16].copy_from_slice(&1u32.to_be_bytes());
        let mut tkhd_text = vec![0u8; 84];
        tkhd_text[12..16].copy_from_slice(&2u32.to_be_bytes());
        let audio = atom(
            b"trak",
            &[
                atom(b"tkhd", &tkhd_audio),
                atom(b"tref", &atom(b"chap", &2u32.to_be_bytes())),
            ]
            .concat(),
        );
        let mut mdhd = vec![0u8; 24];
        mdhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        let be32 = |v: &[u32]| v.iter().flat_map(|n| n.to_be_bytes()).collect::<Vec<u8>>();
        let stbl = [
            full(b"stts", &be32(&[2, 2, 90_000, 1, 5_000])),
            full(b"stsz", &be32(&[0, 3, 9, 9, 9])),
            full(b"stsc", &be32(&[1, 1, 3, 1])),
            full(b"stco", &be32(&[1, 5000])),
        ]
        .concat();
        let text = atom(
            b"trak",
            &[
                atom(b"tkhd", &tkhd_text),
                atom(
                    b"mdia",
                    &[atom(b"mdhd", &mdhd), atom(b"minf", &atom(b"stbl", &stbl))].concat(),
                ),
            ]
            .concat(),
        );
        let moov = [atom(b"mvhd", &mvhd), udta, audio, text].concat();

        let (tags, track) = mp4(&moov);
        assert_eq!(tags.title.as_deref(), Some("Part 1"));
        assert_eq!(tags.album.as_deref(), Some("Dune"));
        assert_eq!(tags.author(), Some("Frank Herbert"));
        assert_eq!(tags.narrator.as_deref(), Some("Scott Brick"));
        assert_eq!(tags.duration, Some(3725.5));
        assert_eq!(
            tags.chapters[1],
            Chapter {
                title: "Part one".into(),
                start: 60.0
            }
        );
        let track = track.expect("chapter track is referenced from tref/chap");
        assert_eq!(
            track.samples,
            vec![(5000, 9, 0.0), (5009, 9, 90.0), (5018, 9, 180.0)]
        );
        assert_eq!(text_sample(b"\x00\x05Intro\x00\x00\x00\x0cencd"), "Intro");
    }

    fn vorbis(entries: &[&str]) -> Vec<u8> {
        let mut b = 4u32.to_le_bytes().to_vec();
        b.extend_from_slice(b"test");
        b.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for e in entries {
            b.extend_from_slice(&(e.len() as u32).to_le_bytes());
            b.extend_from_slice(e.as_bytes());
        }
        b
    }

    #[test]
    fn flac_and_ogg_read_vorbis_comments_with_chapters() {
        let comment = vorbis(&[
            "TITLE=Глава 1",
            "ALBUM=Идиот",
            "artist=Достоевский",
            "CHAPTER001=00:00:00.000",
            "CHAPTER001NAME=Начало",
            "CHAPTER002=00:01:30.500",
            "CHAPTER002NAME=Дальше",
        ]);
        let mut streaminfo = vec![0u8; 34];
        let rate: u64 = 44_100;
        let samples: u64 = 44_100 * 90;
        let packed = (rate << 44) | samples;
        streaminfo[10..18].copy_from_slice(&packed.to_be_bytes());
        let mut file = b"fLaC".to_vec();
        file.extend_from_slice(&[0, 0, 0, 34]);
        file.extend(streaminfo);
        file.push(0x84);
        file.extend_from_slice(&(comment.len() as u32).to_be_bytes()[1..]);
        file.extend(&comment);

        let tags = flac(&file);
        assert_eq!(tags.album.as_deref(), Some("Идиот"));
        assert_eq!(tags.artist.as_deref(), Some("Достоевский"));
        assert_eq!(tags.duration, Some(90.0));
        assert_eq!(
            tags.chapters[1],
            Chapter {
                title: "Дальше".into(),
                start: 90.5
            }
        );

        let page = |packet: &[u8]| {
            let mut p = b"OggS".to_vec();
            p.extend_from_slice(&[0; 22]);
            let mut table = vec![255u8; packet.len() / 255];
            table.push((packet.len() % 255) as u8);
            p.push(table.len() as u8);
            p.extend(table);
            p.extend_from_slice(packet);
            p
        };
        let tags_packet = [&b"OpusTags"[..], &comment].concat();
        let file = [page(b"OpusHead\x01\x02"), page(&tags_packet)].concat();
        let tags = ogg(&file);
        assert_eq!(tags.title.as_deref(), Some("Глава 1"));
        assert_eq!(tags.chapters.len(), 2);
    }

    #[test]
    fn chapters_are_sorted_deduplicated_and_dropped_when_alone() {
        let ch = |t: &str, s: f64| Chapter {
            title: t.into(),
            start: s,
        };
        assert!(tidy(vec![ch("only", 0.0)], 100.0).is_empty());
        assert_eq!(
            tidy(
                vec![
                    ch("b", 50.0),
                    ch("a", 0.0),
                    ch("a2", 0.2),
                    ch("late", 500.0)
                ],
                100.0
            ),
            vec![ch("a", 0.0), ch("b", 50.0)]
        );
    }

    #[test]
    fn broken_input_never_panics() {
        for junk in [
            &b""[..],
            b"ID3",
            b"ID3\x04\x00\x00\x7f\x7f\x7f\x7fTIT2",
            b"fLaC\x84\xff\xff\xff",
            b"OggS",
        ] {
            id3v2(junk);
            id3v1(junk);
            mp4(junk);
            flac(junk);
            ogg(junk);
            atoms(junk);
        }
    }
}
