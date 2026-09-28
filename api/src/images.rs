use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use base64::Engine;
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, ExtendedColorType, ImageFormat};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::AuthUser, AppError, AppState, Result};

const MAX_SIDE: u32 = 1600;
const QUALITY: u8 = 78;
const KEEP: usize = 120 * 1024;
const INLINE: usize = 2 * 1024 * 1024;

pub struct Img {
    pub mime: String,
    pub data: Vec<u8>,
}

pub fn shrink(mime: &str, data: Vec<u8>) -> (String, Vec<u8>) {
    if data.len() <= KEEP {
        return (mime.to_string(), data);
    }
    let Ok(img) = image::load_from_memory(&data) else {
        return (mime.to_string(), data);
    };
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, FilterType::Triangle)
    } else {
        img
    };

    let mut out = Vec::new();
    // ponytail: alpha survives as png, everything else becomes jpeg. A resized
    // png photo stays bigger than jpeg — if that shows up in real books, encode
    // both and keep the smaller.
    let done = if img.color().has_alpha() {
        img.write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png)
            .map(|()| "image/png")
    } else {
        let rgb = img.to_rgb8();
        JpegEncoder::new_with_quality(&mut out, QUALITY)
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
            .map(|()| "image/jpeg")
    };
    match done {
        Ok(kind) if !out.is_empty() && out.len() < data.len() => (kind.to_string(), out),
        _ => (mime.to_string(), data),
    }
}

pub fn smaller(mime: String, b64: String) -> (String, String) {
    if b64.len() <= KEEP {
        return (mime, b64);
    }
    let engine = base64::engine::general_purpose::STANDARD;
    let Ok(data) = engine.decode(&b64) else {
        return (mime, b64);
    };
    let (mime, data) = shrink(&mime, data);
    (mime, engine.encode(data))
}

const MARK: &str = "data:image/";

pub fn extract(doc: String) -> (String, Vec<Img>) {
    if !doc.contains(MARK) || total(&doc) <= INLINE {
        return (doc, Vec::new());
    }

    let engine = base64::engine::general_purpose::STANDARD;
    let mut out = String::with_capacity(doc.len());
    let mut imgs: Vec<Img> = Vec::new();
    let mut rest = doc.as_str();

    while let Some(at) = rest.find(MARK) {
        out.push_str(&rest[..at]);
        let uri = &rest[at..];
        let Some((mime, tail)) = uri.split_once(";base64,") else {
            out.push_str(uri);
            return (out, Vec::new());
        };
        let end = tail.find(['"', '\\', '\'']).unwrap_or(tail.len());
        let (b64, tail) = tail.split_at(end);

        match engine.decode(b64) {
            Ok(data) => {
                out.push_str(&format!("img/{}", imgs.len()));
                imgs.push(Img {
                    mime: mime.trim_start_matches("data:").to_string(),
                    data,
                });
            }
            Err(_) => out.push_str(&uri[..uri.len() - tail.len()]),
        }
        rest = tail;
    }
    out.push_str(rest);
    (out, imgs)
}

fn total(doc: &str) -> usize {
    doc.match_indices(MARK)
        .map(|(at, _)| doc[at..].find(['"', '\\', '\'']).unwrap_or(doc.len() - at))
        .sum()
}

pub async fn token_for(
    State(state): State<Arc<AppState>>,
    _user: AuthUser,
    Path(filename): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let (id, _) = crate::books::parse_filename(&filename)?;
    Ok(Json(serde_json::json!({
        "token": crate::auth::book_token(&state.jwt_secret, id)?
    })))
}

#[derive(Deserialize)]
pub struct Signed {
    t: String,
}

pub async fn one(
    State(state): State<Arc<AppState>>,
    Path((id, idx)): Path<(Uuid, i32)>,
    Query(signed): Query<Signed>,
) -> Result<impl IntoResponse> {
    if crate::auth::book_of_token(&state.jwt_secret, &signed.t) != Some(id) {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "Нужен вход",
            "Not authorized",
        ));
    }

    let row: Option<(String, Vec<u8>)> =
        sqlx::query_as("select mime, data from book_images where book_id = $1 and idx = $2")
            .bind(id)
            .bind(idx)
            .fetch_optional(&state.db)
            .await?;

    let (mime, data) = row.ok_or_else(|| {
        AppError::new(
            StatusCode::NOT_FOUND,
            "Картинка не найдена",
            "Image not found",
        )
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=86400".to_string()),
        ],
        data,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 253) as u8, ((x * y) % 249) as u8])
        });
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 95)
            .encode(img.as_raw(), w, h, ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    #[test]
    fn big_scan_shrinks_small_picture_is_left_alone() {
        let scan = jpeg(2400, 3200);
        assert!(scan.len() > KEEP);
        let (mime, small) = shrink("image/jpeg", scan.clone());
        assert_eq!(mime, "image/jpeg");
        assert!(
            small.len() * 4 < scan.len(),
            "{} из {}",
            small.len(),
            scan.len()
        );
        assert_eq!(
            image::load_from_memory(&small).unwrap().height(),
            MAX_SIDE,
            "длинная сторона должна стать потолком"
        );

        let thumb = jpeg(40, 40);
        assert_eq!(shrink("image/jpeg", thumb.clone()).1, thumb);

        let broken = vec![1u8, 2, 3];
        assert_eq!(shrink("image/png", broken.clone()).1, broken);
    }

    #[test]
    fn heavy_document_moves_images_out_light_one_keeps_them() {
        let engine = base64::engine::general_purpose::STANDARD;
        let heavy = engine.encode(vec![7u8; INLINE]);
        let doc = format!(
            r#"{{"chapters":[{{"html":"<p>a</p><img src=\"data:image/jpeg;base64,{heavy}\"><img src=\"data:image/png;base64,{heavy}\">"}}]}}"#
        );

        let (out, imgs) = extract(doc);
        assert_eq!(imgs.len(), 2);
        assert_eq!(imgs[0].mime, "image/jpeg");
        assert_eq!(imgs[1].mime, "image/png");
        assert_eq!(imgs[0].data, vec![7u8; INLINE]);
        assert!(
            out.contains(r#"src=\"img/0\""#),
            "{}",
            &out[..80.min(out.len())]
        );
        assert!(out.contains(r#"src=\"img/1\""#));
        assert!(!out.contains("base64"));

        let light = r#"{"chapters":[{"html":"<img src=\"data:image/png;base64,QUJD\">"}]}"#;
        let (same, none) = extract(light.to_string());
        assert_eq!(same, light);
        assert!(none.is_empty());
    }
}
