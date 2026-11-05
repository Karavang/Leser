use quick_xml::events::Event;

use crate::{AppError, Result};

const BLOCKS: [&[u8]; 16] = [
    b"p",
    b"l",
    b"head",
    b"stage",
    b"speaker",
    b"titlePart",
    b"ab",
    b"castItem",
    b"item",
    b"docAuthor",
    b"byline",
    b"dateline",
    b"salute",
    b"signed",
    b"trailer",
    b"opener",
];
const SKIP: [&[u8]; 4] = [b"note", b"teiHeader", b"standOff", b"figDesc"];

pub struct Tei {
    pub title: Option<String>,
    pub chapters: Vec<String>,
}

// ponytail: a new chapter starts at every <head> that follows text, so a play splits into scenes;
// footnotes are dropped, inline markup is flattened to plain text.
pub fn read(xml: &[u8]) -> Result<Tei> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut title: Option<String> = None;
    let mut heading = String::new();
    let mut para: Option<String> = None;
    let mut chapter: Vec<String> = Vec::new();
    let mut has_text = false;
    let mut chapters: Vec<String> = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| {
            AppError::bad(
                format!("Не удалось прочитать TEI: {e}"),
                format!("Could not read TEI: {e}"),
            )
        })?;
        match event {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_vec();
                let inside = in_text(&stack);
                if inside && BLOCKS.contains(&name.as_slice()) {
                    flush(&mut para, &mut chapter, &mut has_text);
                    para = Some(String::new());
                }
                stack.push(name);
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_vec();
                if let Some(at) = stack.iter().rposition(|n| *n == name) {
                    stack.truncate(at);
                }
                if name == b"title" && title.is_none() && !heading.trim().is_empty() {
                    title = Some(clean(&heading));
                }
                if in_text(&stack) || name == b"text" {
                    if name == b"head" {
                        if has_text {
                            chapters.push(chapter.join("\n"));
                            chapter.clear();
                            has_text = false;
                        }
                        if let Some(p) = para.take().map(|p| clean(&p)).filter(|p| !p.is_empty()) {
                            chapter.push(p);
                        }
                    } else if BLOCKS.contains(&name.as_slice()) {
                        flush(&mut para, &mut chapter, &mut has_text);
                    }
                }
            }
            Event::Empty(e) if e.local_name().as_ref() == b"lb" => {
                if let Some(p) = para.as_mut() {
                    p.push(' ');
                }
            }
            Event::Text(t) => {
                let Ok(t) = t.unescape() else { continue };
                if title.is_none() && is_title(&stack) {
                    heading.push_str(&t);
                } else if in_text(&stack) {
                    if let Some(p) = para.as_mut() {
                        p.push_str(&t);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    flush(&mut para, &mut chapter, &mut has_text);
    if !chapter.is_empty() {
        chapters.push(chapter.join("\n"));
    }
    if !chapters.iter().any(|c| !c.is_empty()) {
        return Err(AppError::bad("В TEI нет текста", "No text in TEI"));
    }
    Ok(Tei {
        title: title.map(|t| t.split(" : ELTeC").next().unwrap_or(&t).trim().to_string()),
        chapters,
    })
}

fn in_text(stack: &[Vec<u8>]) -> bool {
    stack.iter().any(|n| n == b"text") && !stack.iter().any(|n| SKIP.contains(&n.as_slice()))
}

fn is_title(stack: &[Vec<u8>]) -> bool {
    stack.last().is_some_and(|n| n == b"title") && stack.iter().any(|n| n == b"titleStmt")
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flush(para: &mut Option<String>, chapter: &mut Vec<String>, has_text: &mut bool) {
    if let Some(p) = para.take().map(|p| clean(&p)).filter(|p| !p.is_empty()) {
        chapter.push(p);
        *has_text = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapters_follow_heads_and_notes_are_dropped() {
        let xml = r#"<TEI xmlns="http://www.tei-c.org/ns/1.0"><teiHeader><fileDesc><titleStmt>
            <title>Сердешна Оксана : ELTeC edition</title><author>Квітка</author></titleStmt></fileDesc></teiHeader>
            <text><front><p>(Любій моїй жінці)</p></front><body>
            <div><head>Глава I</head><p>Нема на
              світі<note>сноска</note> нічого<lb/>луччого &amp; милішого.</p><p>Друге.</p></div>
            <div><head>Действие</head><head>Явление I</head><sp><speaker>ГОРОДНИЧИЙ.</speaker><p>Я пригласил вас.</p></sp>
            <stage>Уходит.</stage></div></body></text></TEI>"#;
        let tei = read(xml.as_bytes()).unwrap();
        assert_eq!(tei.title.as_deref(), Some("Сердешна Оксана"));
        assert_eq!(
            tei.chapters,
            vec![
                "(Любій моїй жінці)",
                "Глава I\nНема на світі нічого луччого & милішого.\nДруге.",
                "Действие\nЯвление I\nГОРОДНИЧИЙ.\nЯ пригласил вас.\nУходит.",
            ]
        );
    }

    #[test]
    fn empty_or_broken_tei_is_an_error() {
        assert!(read(b"<TEI><teiHeader/><text><body></body></text></TEI>").is_err());
        assert!(read(b"not xml at all").is_err());
    }
}
