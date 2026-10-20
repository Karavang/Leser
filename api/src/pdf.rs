use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};

pub struct Glyph {
    pub x: f64,
    pub y: f64,
    pub end: f64,
    pub size: f64,
    pub ch: String,
}

#[derive(Default)]
pub struct Pages {
    pub pages: Vec<Vec<Glyph>>,
    height: f64,
}

impl OutputDev for Pages {
    fn begin_page(
        &mut self,
        _: u32,
        media: &MediaBox,
        _: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.height = media.ury - media.lly;
        self.pages.push(Vec::new());
        Ok(())
    }
    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _: f64,
        font_size: f64,
        ch: &str,
    ) -> Result<(), OutputError> {
        let vx = font_size * (trm.m11 + trm.m21);
        let vy = font_size * (trm.m12 + trm.m22);
        let size = (vx * vy).abs().sqrt();
        let x = trm.m31;
        if let Some(page) = self.pages.last_mut() {
            page.push(Glyph {
                x,
                y: self.height - trm.m32,
                end: x + width * size,
                size,
                ch: ch.to_string(),
            });
        }
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

pub fn glyphs(bytes: &[u8]) -> Result<Pages, OutputError> {
    let mut doc = pdf_extract::Document::load_mem(bytes)?;
    if doc.is_encrypted() {
        doc.decrypt("")?;
    }
    let mut pages = Pages::default();
    pdf_extract::output_doc(&doc, &mut pages)?;
    Ok(pages)
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Heading(u8, String),
    Para(String),
}

struct Line {
    text: String,
    x0: f64,
    y: f64,
    size: f64,
}

fn lines(page: &[Glyph]) -> Vec<Line> {
    let mut runs: Vec<Vec<&Glyph>> = Vec::new();
    for g in page {
        let same = runs
            .last()
            .and_then(|r| r.last())
            .is_some_and(|last| (g.y - last.y).abs() <= g.size * 0.5 && g.x >= last.end - g.size);
        if same {
            runs.last_mut().expect("checked above").push(g);
        } else {
            runs.push(vec![g]);
        }
    }
    let mut out = Vec::new();
    for run in runs {
        let spaced = run.iter().any(|g| g.ch == " ");
        let mut text = String::new();
        let mut prev: Option<&Glyph> = None;
        for g in &run {
            if let Some(p) = prev {
                let gap = g.x - p.end;
                if p.ch != " " && gap > g.size * if spaced { 3.5 } else { 0.15 } {
                    text.push(' ');
                }
            }
            text.push_str(if g.ch.trim().is_empty() { " " } else { &g.ch });
            prev = Some(g);
        }
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let inked: Vec<&&Glyph> = run.iter().filter(|g| !g.ch.trim().is_empty()).collect();
        let Some(first) = inked.first() else {
            continue;
        };
        out.push(Line {
            text,
            x0: first.x,
            y: first.y,
            size: inked.iter().map(|g| g.size).fold(0.0, f64::max),
        });
    }
    out
}

fn mode(values: impl Iterator<Item = (f64, usize)>) -> f64 {
    let mut counts = std::collections::HashMap::<i64, usize>::new();
    for (v, w) in values {
        *counts.entry((v * 2.0).round() as i64).or_default() += w;
    }
    counts
        .into_iter()
        .max_by_key(|&(v, n)| (n, std::cmp::Reverse(v)))
        .map(|(v, _)| v as f64 / 2.0)
        .unwrap_or(0.0)
}

pub fn blocks(pages: &Pages) -> Vec<Block> {
    let pages: Vec<Vec<Line>> = pages.pages.iter().map(|p| lines(p)).collect();

    let body = mode(pages.iter().flatten().map(|l| (l.size, l.text.len())));
    let is_body = |l: &Line| (l.size - body).abs() <= body * 0.1;
    let body_lines: Vec<&Line> = pages.iter().flatten().filter(|l| is_body(l)).collect();
    let hyphens = body_lines.iter().filter(|l| l.text.ends_with('-')).count();
    let hyphenated = hyphens >= 20 && hyphens * 50 > body_lines.len();
    let step = {
        let mut steps: Vec<f64> = pages
            .iter()
            .flat_map(|p| p.windows(2).map(|w| w[1].y - w[0].y))
            .filter(|d| *d > 0.0)
            .collect();
        steps.sort_by(f64::total_cmp);
        steps.get(steps.len() / 2).copied().unwrap_or(body * 1.2)
    };

    let edge_key = |l: &Line| -> String {
        l.text
            .chars()
            .filter(|c| !c.is_ascii_digit())
            .collect::<String>()
            .trim()
            .to_string()
    };
    let mut edges = std::collections::HashMap::<String, usize>::new();
    for page in &pages {
        for l in page.first().into_iter().chain(page.last()) {
            *edges.entry(edge_key(l)).or_default() += 1;
        }
    }

    let mut out: Vec<Block> = Vec::new();
    let mut para = String::new();
    let flush = |para: &mut String, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(std::mem::take(para)));
        }
    };
    for page in &pages {
        let left = page
            .iter()
            .filter(|l| is_body(l))
            .map(|l| l.x0)
            .fold(f64::INFINITY, f64::min);
        let last = page.len().saturating_sub(1);
        let mut prev_y: Option<f64> = None;
        let mut prev_heading = false;
        let mut prev_small = false;
        for (i, line) in page.iter().enumerate() {
            let key = edge_key(line);
            let at_edge = i == 0 || i == last;
            let repeated = key.is_empty() || edges.get(&key).is_some_and(|n| *n >= 3);
            if at_edge && repeated && line.size <= body * 1.1 {
                continue;
            }
            if line.size > body * 1.15 && line.text.chars().count() < 120 {
                flush(&mut para, &mut out);
                let level = if line.size >= body * 1.6 { 2 } else { 3 };
                match out.last_mut() {
                    Some(Block::Heading(l, text)) if prev_heading && *l == level => {
                        text.push(' ');
                        text.push_str(&line.text);
                    }
                    _ => out.push(Block::Heading(level, line.text.clone())),
                }
                prev_heading = true;
                prev_y = Some(line.y);
                continue;
            }
            let indent = left.is_finite() && line.x0 > left + line.size * 0.8;
            let gap = prev_y.is_some_and(|y| line.y - y > step * 1.6);
            let small = line.size < body * 0.85;
            if indent || gap || prev_heading || small != prev_small {
                flush(&mut para, &mut out);
            }
            let starts_lower = line.text.chars().next().is_some_and(char::is_lowercase);
            if para.ends_with('-') {
                if hyphenated && starts_lower {
                    para.pop();
                }
            } else if !para.is_empty() {
                para.push(' ');
            }
            match line.text.strip_prefix('—') {
                Some(rest) if para.is_empty() && !rest.starts_with(' ') => {
                    para.push_str("— ");
                    para.push_str(rest);
                }
                _ => para.push_str(&line.text),
            }
            prev_small = small;
            prev_heading = false;
            prev_y = Some(line.y);
        }
    }
    flush(&mut para, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(page: &mut Vec<Glyph>, x: f64, y: f64, size: f64, text: &str, kern: f64) {
        let mut at = x;
        for ch in text.chars() {
            let w = if ch == ' ' { 0.25 } else { 0.5 };
            page.push(Glyph {
                x: at,
                y,
                end: at + w * size,
                size,
                ch: ch.to_string(),
            });
            at += w * size + if ch == ' ' { 0.0 } else { kern * size };
        }
    }

    #[test]
    fn running_heads_are_dropped_and_small_print_stays_apart() {
        let pages = (1..=3)
            .map(|n| {
                let mut p = Vec::new();
                line(
                    &mut p,
                    200.0,
                    40.0,
                    15.0,
                    &format!("Ніколи не здригайся {n}"),
                    0.0,
                );
                line(&mut p, 105.0, 80.0, 15.0, &format!("Абзац {n}"), 0.0);
                line(&mut p, 77.0, 97.0, 15.0, "сторінки.", 0.0);
                let note = ["Перша примітка.", "Друга примітка.", "Третя примітка."][n - 1];
                line(&mut p, 77.0, 600.0, 11.0, note, 0.0);
                p
            })
            .collect();
        let got = blocks(&Pages { pages, height: 0.0 });
        assert_eq!(
            got,
            [
                Block::Para("Абзац 1 сторінки.".into()),
                Block::Para("Перша примітка.".into()),
                Block::Para("Абзац 2 сторінки.".into()),
                Block::Para("Друга примітка.".into()),
                Block::Para("Абзац 3 сторінки.".into()),
                Block::Para("Третя примітка.".into()),
            ]
        );
    }

    #[test]
    fn lines_without_space_glyphs_split_words_by_gaps() {
        let mut page = Vec::new();
        let mut at = 10.0;
        for word in ["Asterios", "Bakolas"] {
            for ch in word.chars() {
                page.push(Glyph {
                    x: at,
                    y: 50.0,
                    end: at + 5.0,
                    size: 10.0,
                    ch: ch.to_string(),
                });
                at += 5.0;
            }
            at += 3.0;
        }
        assert_eq!(lines(&page)[0].text, "Asterios Bakolas");
    }

    #[test]
    fn geometry_makes_paragraphs_headings_and_ignores_kerning() {
        let mut p1 = Vec::new();
        line(&mut p1, 77.0, 80.0, 15.0, "1", 0.0);
        line(&mut p1, 200.0, 110.0, 30.0, "ТРІҐ", 0.0);
        line(&mut p1, 105.0, 150.0, 15.0, "Коло правди збирається", 0.3);
        line(&mut p1, 77.0, 167.0, 15.0, "щодня, але сьогодні", 0.0);
        let mut p2 = Vec::new();
        line(&mut p2, 77.0, 80.0, 15.0, "була особлива ситуація.", 0.0);
        line(&mut p2, 105.0, 97.0, 15.0, "—Я Тріґ.", 0.0);
        line(&mut p2, 105.0, 114.0, 15.0, "—Привіт, Тріґу, заклик-", 0.6);
        line(&mut p2, 77.0, 131.0, 15.0, "відповідь.", 0.0);
        line(&mut p2, 77.0, 148.0, 15.0, "2", 0.0);

        let got = blocks(&Pages {
            pages: vec![p1, p2],
            height: 0.0,
        });
        assert_eq!(
            got,
            [
                Block::Heading(2, "ТРІҐ".into()),
                Block::Para(
                    "Коло правди збирається щодня, але сьогодні була особлива ситуація.".into()
                ),
                Block::Para("— Я Тріґ.".into()),
                Block::Para("— Привіт, Тріґу, заклик-відповідь.".into()),
            ]
        );
    }
}
