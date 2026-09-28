// ponytail: nothing calls this yet — the queue and the worker come next.
#![allow(dead_code)]

use crate::reader::numeric_label;

const RAMP: [usize; 3] = [700, 1600, 3000];
const MAX: usize = 3000;
const MIN: usize = 250;

#[derive(Debug, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub para: usize,
}

pub fn pieces(html: &str) -> Vec<Piece> {
    let paras = paragraphs(html);
    let mut out: Vec<Piece> = Vec::new();
    let mut buf = String::new();
    let mut first = 0;

    for (i, para) in paras.iter().enumerate() {
        let mut rest = para.as_str();
        loop {
            let used = buf.chars().count();
            let room = budget(out.len()).saturating_sub(used + if used == 0 { 0 } else { 2 });

            if rest.chars().count() <= room {
                add(&mut buf, &mut first, i, rest);
                break;
            }
            if used >= MIN || room == 0 {
                flush(&mut out, &mut buf, first);
                continue;
            }
            let (head, tail) = fit(rest, room);
            add(&mut buf, &mut first, i, head);
            flush(&mut out, &mut buf, first);
            rest = tail;
        }
    }
    flush(&mut out, &mut buf, first);
    out
}

fn budget(done: usize) -> usize {
    *RAMP.get(done).unwrap_or(&MAX)
}

fn add(buf: &mut String, first: &mut usize, para: usize, text: &str) {
    if buf.is_empty() {
        *first = para;
    } else {
        buf.push_str("\n\n");
    }
    buf.push_str(text);
}

fn flush(out: &mut Vec<Piece>, buf: &mut String, para: usize) {
    let text = std::mem::take(buf);
    if !text.trim().is_empty() {
        out.push(Piece { text, para });
    }
}

fn fit(text: &str, room: usize) -> (&str, &str) {
    let mut end = text.len();
    let mut stop = None;
    let mut space = None;

    for (n, (i, c)) in text.char_indices().enumerate() {
        if n == room {
            end = i;
            break;
        }
        if c == ' ' {
            space = Some(i);
            if text[..i]
                .chars()
                .next_back()
                .is_some_and(|p| ".!?…»".contains(p))
            {
                stop = Some(i);
            }
        }
    }
    let at = stop.or(space).filter(|n| *n > 0).unwrap_or(end);
    (text[..at].trim_end(), text[at..].trim_start())
}

const BLOCK: [&str; 13] = [
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "blockquote",
    "div",
    "td",
    "pre",
    "figcaption",
];

fn paragraphs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut link = String::new();
    let mut in_link = false;
    let mut sup = 0usize;
    let mut rest = html;

    loop {
        let Some(lt) = rest.find('<') else {
            push(&mut buf, &mut link, in_link, sup, rest);
            break;
        };
        push(&mut buf, &mut link, in_link, sup, &rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            break;
        };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];

        let closing = tag.starts_with('/');
        let void = tag.ends_with('/');
        let name = tag
            .trim_start_matches('/')
            .split([' ', '\t', '\n', '/'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        match name.as_str() {
            "sup" if closing => sup = sup.saturating_sub(1),
            "sup" if !void => sup += 1,
            // ponytail: a link whose whole text is a number is a footnote mark;
            // real link text never looks like "[12]". Drop the mark, keep the
            // link text. If some book proves otherwise, match the href instead.
            "a" if closing => {
                in_link = false;
                let text = std::mem::take(&mut link);
                if !numeric_label(&text) {
                    buf.push_str(&text);
                }
            }
            "a" if !void => in_link = true,
            "br" => end(&mut out, &mut buf),
            n if BLOCK.contains(&n) && closing => end(&mut out, &mut buf),
            _ => {}
        }
    }
    end(&mut out, &mut buf);
    out
}

fn push(buf: &mut String, link: &mut String, in_link: bool, sup: usize, text: &str) {
    if sup > 0 || text.is_empty() {
        return;
    }
    if in_link { link } else { buf }.push_str(text);
}

fn end(out: &mut Vec<String>, buf: &mut String) {
    let text = clean(&std::mem::take(buf));
    if text.chars().any(|c| c.is_alphanumeric()) {
        out.push(text);
    }
}

fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.chars() {
        if c.is_whitespace() {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    out.trim()
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paras(p: &[Piece]) -> Vec<&str> {
        p.iter().flat_map(|x| x.text.split("\n\n")).collect()
    }

    #[test]
    fn first_piece_is_short_then_pieces_grow() {
        let para = "Очень обычный абзац из ста знаков, ровно такой, какие и бывают в книгах у живых людей. ";
        let html: String = (0..40).map(|_| format!("<p>{}</p>", para.trim())).collect();

        let got = pieces(&html);
        let sizes: Vec<usize> = got.iter().map(|p| p.text.chars().count()).collect();

        assert!(sizes[0] <= RAMP[0], "первый кусок {} знаков", sizes[0]);
        assert!(sizes[1] <= RAMP[1]);
        assert!(sizes[2..].iter().all(|n| *n <= MAX));
        assert!(sizes[0] >= RAMP[0] / 2, "первый кусок вышел пустым");
        assert_eq!(paras(&got).len(), 40, "абзацы не должны теряться");
        assert!(paras(&got).iter().all(|p| *p == para.trim()));
        assert_eq!(got[0].para, 0);
        assert!(got[1].para > 0);
    }

    #[test]
    fn long_paragraph_breaks_on_sentences() {
        let s = "Он шёл по улице и думал о том, что всё это когда-нибудь кончится. ";
        let html = format!("<p>{}</p>", s.repeat(80));

        let got = pieces(&html);
        assert!(got.len() > 1);
        assert!(got.iter().all(|p| p.text.chars().count() <= MAX));
        assert!(
            got[..got.len() - 1].iter().all(|p| p.text.ends_with('.')),
            "куски должны кончаться на границе предложения"
        );
        assert!(got.iter().all(|p| p.para == 0));
    }

    #[test]
    fn footnotes_and_markup_do_not_reach_the_voice() {
        let html = r##"<h2>Глава 1</h2><p>Текст<sup>12</sup> и <a href="#a3">[4]</a> дальше &amp; ещё,
             <em>с переносом</em> внутри.</p><p>Ссылка на <a href="#a9">другую главу</a>.</p>
             <span id="a1"></span><img src="img:1"><blockquote><p>Цитата</p></blockquote>"##;

        assert_eq!(
            paras(&pieces(html)),
            [
                "Глава 1",
                "Текст и дальше & ещё, с переносом внутри.",
                "Ссылка на другую главу.",
                "Цитата",
            ]
        );
    }

    #[test]
    fn empty_chapter_gives_nothing() {
        assert!(pieces("").is_empty());
        assert!(pieces("<p> </p><img src=\"img:1\">").is_empty());
    }
}

