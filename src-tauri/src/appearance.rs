//! Local appearance preferences and bounded, static PNG sticker preparation.
use crate::store::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{
    codecs::png::{PngDecoder, PngEncoder},
    DynamicImage, ImageDecoder, ImageEncoder, Limits,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

pub const MAX_INPUT_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_STICKER_BYTES: usize = 512 * 1024;
const PNG_PREFIX: &str = "data:image/png;base64,";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub accent: Option<String>,
    pub background: String,
    pub color_start: String,
    pub color_end: String,
    pub angle: u16,
    pub sticker_enabled: bool,
    pub sticker_name: Option<String>,
    pub sticker_data_url: Option<String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: None,
            background: "theme".into(),
            color_start: "#d9a7c7".into(),
            color_end: "#fffcdc".into(),
            angle: 90,
            sticker_enabled: false,
            sticker_name: None,
            sticker_data_url: None,
        }
    }
}

fn color(value: &str) -> Result<String> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("颜色需为六位 HEX，例如 #ff512f。".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err("贴纸名称需为 1–80 个字符，不能包含控制字符。".into());
    }
    Ok(value.to_string())
}

fn decode_base64(value: &str, max_bytes: usize) -> Result<Vec<u8>> {
    if value.len() > max_bytes.div_ceil(3) * 4 {
        return Err("贴纸文件超过大小限制。".into());
    }
    let bytes = STANDARD.decode(value).map_err(|_| "贴纸 PNG 编码无效。")?;
    if bytes.len() > max_bytes {
        return Err("贴纸文件超过大小限制。".into());
    }
    Ok(bytes)
}

fn decode_png(bytes: &[u8], max_edge: u32, max_pixels: u64) -> Result<DynamicImage> {
    if bytes.len() < 45
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || &bytes[8..16] != b"\0\0\0\rIHDR"
        || !bytes.ends_with(b"\0\0\0\0IEND\xaeB`\x82")
    {
        return Err("贴纸必须是完整的静态 PNG 图片。".into());
    }
    // Check chunk boundaries before decode, including duplicate IEND/trailing data and APNG.
    let mut offset = 8usize;
    loop {
        if bytes.len().saturating_sub(offset) < 12 {
            return Err("贴纸 PNG 数据块不完整。".into());
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or("贴纸 PNG 数据块超出文件范围。")?;
        let kind = &bytes[offset + 4..offset + 8];
        if kind == b"acTL" {
            return Err("贴纸仅支持静态 PNG，不支持动画 PNG。".into());
        }
        if kind == b"IEND" {
            if length != 0 || end != bytes.len() {
                return Err("贴纸 PNG 结束后含有多余数据。".into());
            }
            break;
        }
        offset = end;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if width == 0
        || height == 0
        || width > max_edge
        || height > max_edge
        || u64::from(width) * u64::from(height) > max_pixels
    {
        return Err("贴纸图片尺寸超过限制：最长边 2048，最多 400 万像素。".into());
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_edge);
    limits.max_image_height = Some(max_edge);
    limits.max_alloc = Some(32 * 1024 * 1024);
    let decoder = PngDecoder::with_limits(Cursor::new(bytes), limits)
        .map_err(|_| "贴纸 PNG 损坏或超出解码限制。")?;
    if decoder.total_bytes() > 32 * 1024 * 1024
        || decoder.is_apng().map_err(|_| "无法读取贴纸格式。")?
    {
        return Err("贴纸仅支持静态 PNG，不支持动画 PNG。".into());
    }
    DynamicImage::from_decoder(decoder).map_err(|_| "贴纸 PNG 像素损坏。".into())
}

impl Appearance {
    pub fn validate(&self) -> Result<()> {
        if let Some(accent) = &self.accent {
            color(accent)?;
        }
        color(&self.color_start)?;
        color(&self.color_end)?;
        if !["theme", "solid", "gradient"].contains(&self.background.as_str()) || self.angle > 359 {
            return Err("背景模式或渐变方向无效，角度需为 0–359 的整数。".into());
        }
        match (&self.sticker_name, &self.sticker_data_url) {
            (Some(label), Some(url)) => {
                name(label)?;
                let base64 = url
                    .strip_prefix(PNG_PREFIX)
                    .ok_or("贴纸必须是本地 PNG 数据。")?;
                let bytes = decode_base64(base64, MAX_STICKER_BYTES)?;
                decode_png(&bytes, 512, 512 * 512)?;
            }
            (None, None) if !self.sticker_enabled => {}
            _ => return Err("贴纸数据不完整，未更改外观设置。".into()),
        }
        Ok(())
    }

    pub fn patched(&self, patch: &serde_json::Value) -> Result<Self> {
        let fields = patch.as_object().ok_or("外观修改需为对象。")?;
        let mut next = self.clone();
        for (key, value) in fields {
            match key.as_str() {
                "accent" => {
                    next.accent = if value.is_null() {
                        None
                    } else {
                        Some(color(value.as_str().ok_or("主题色需为 HEX 或 null。")?)?)
                    }
                }
                "background" => {
                    next.background = value.as_str().ok_or("背景模式无效。")?.to_string()
                }
                "colorStart" => next.color_start = color(value.as_str().ok_or("渐变颜色无效。")?)?,
                "colorEnd" => next.color_end = color(value.as_str().ok_or("渐变颜色无效。")?)?,
                "angle" => {
                    next.angle = value
                        .as_u64()
                        .filter(|angle| *angle <= 359)
                        .ok_or("渐变角度需为 0–359 的整数。")?
                        as u16
                }
                "stickerEnabled" => {
                    next.sticker_enabled = value.as_bool().ok_or("贴纸开关无效。")?
                }
                _ => return Err("不支持的外观修改字段；贴纸需通过导入或移除操作更改。".into()),
            }
        }
        next.validate()?;
        Ok(next)
    }

    pub fn with_sticker(&self, sticker: PreparedSticker) -> Self {
        Self {
            sticker_enabled: true,
            sticker_name: Some(sticker.name),
            sticker_data_url: Some(sticker.data_url),
            ..self.clone()
        }
    }

    pub fn without_sticker(&self) -> Self {
        Self {
            sticker_enabled: false,
            sticker_name: None,
            sticker_data_url: None,
            ..self.clone()
        }
    }
}

pub struct PreparedSticker {
    name: String,
    data_url: String,
}

pub fn prepare_sticker(png_base64: &str, label: &str) -> Result<PreparedSticker> {
    let name = name(label)?;
    let bytes = decode_base64(png_base64, MAX_INPUT_BYTES)?;
    let original = decode_png(&bytes, 2048, 4_000_000)?;
    let resized = if original.width() > 512 || original.height() > 512 {
        original.resize(512, 512, image::imageops::FilterType::Lanczos3)
    } else {
        original
    };
    let rgba = resized.into_rgba8();
    let mut output = Vec::new();
    // Re-encoding keeps alpha and removes ancillary metadata; no paths or remote assets are retained.
    PngEncoder::new(&mut output)
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|_| "无法处理贴纸 PNG。")?;
    if output.len() > MAX_STICKER_BYTES {
        return Err("处理后的贴纸仍超过 512 KiB，请选择更小或更简单的 PNG。".into());
    }
    Ok(PreparedSticker {
        name,
        data_url: format!("{PNG_PREFIX}{}", STANDARD.encode(output)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn sample_png(width: u32, height: u32) -> String {
        let rgba: Vec<u8> = (0..width * height)
            .flat_map(|i| [140, 80, 210, if i % 2 == 0 { 0 } else { 255 }])
            .collect();
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .unwrap();
        STANDARD.encode(png)
    }

    fn insert_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        let payload = [kind.as_slice(), data].concat();
        let mut crc = !0u32;
        for byte in &payload {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
            }
        }
        let chunk = [
            &(data.len() as u32).to_be_bytes()[..],
            &payload,
            &(!crc).to_be_bytes()[..],
        ]
        .concat();
        png.splice(33..33, chunk);
    }

    #[test]
    fn patches_reject_invalid_values_and_asset_injection() {
        let original = Appearance::default();
        assert!(original.validate().is_ok());
        let next = original
            .patched(&json!({"accent":"#AABBCC","background":"gradient","angle":359}))
            .unwrap();
        assert_eq!(next.accent.as_deref(), Some("#aabbcc"));
        assert_eq!(
            next.patched(&json!({"colorStart":"#123456"}))
                .unwrap()
                .accent,
            next.accent
        );
        assert_eq!(next.patched(&json!({"accent":null})).unwrap().accent, None);
        for patch in [
            json!([]),
            json!({"accent":"red"}),
            json!({"accent":"#abcd"}),
            json!({"background":"url(file://private)"}),
            json!({"colorEnd":null}),
            json!({"angle":-1}),
            json!({"angle":360}),
            json!({"angle":1.5}),
            json!({"stickerEnabled":true}),
            json!({"stickerEnabled":null}),
            json!({"stickerDataUrl":"data:image/svg+xml,<svg/>"}),
            json!({"stickerName":"hidden"}),
        ] {
            assert!(original.patched(&patch).is_err(), "accepted {patch}");
        }
    }

    #[test]
    fn sticker_preparation_preserves_alpha_and_rejects_untrusted_or_oversized_data() {
        let small = prepare_sticker(&sample_png(2, 1), "  我的贴纸.png  ").unwrap();
        let state = Appearance::default().with_sticker(small);
        assert!(state.validate().is_ok());
        assert_eq!(state.sticker_name.as_deref(), Some("我的贴纸.png"));
        let png = STANDARD
            .decode(
                state
                    .sticker_data_url
                    .as_ref()
                    .unwrap()
                    .strip_prefix(PNG_PREFIX)
                    .unwrap(),
            )
            .unwrap();
        let rgba = decode_png(&png, 512, 512 * 512).unwrap().into_rgba8();
        assert_eq!(rgba.as_raw(), &[140, 80, 210, 0, 140, 80, 210, 255]);
        let large = prepare_sticker(&sample_png(1024, 256), "星星").unwrap();
        let png = STANDARD
            .decode(large.data_url.strip_prefix(PNG_PREFIX).unwrap())
            .unwrap();
        let image = decode_png(&png, 512, 512 * 512).unwrap();
        assert_eq!((image.width(), image.height()), (512, 128));
        assert!(prepare_sticker("not base64", "test").is_err());
        assert!(prepare_sticker(&STANDARD.encode(b"<svg/>"), "test.png").is_err());
        assert!(prepare_sticker(&sample_png(2, 1), "a\nb").is_err());
        assert!(prepare_sticker(&sample_png(2, 1), &"x".repeat(81)).is_err());
        assert!(prepare_sticker(&"A".repeat(MAX_INPUT_BYTES.div_ceil(3) * 4 + 1), "test").is_err());
        let mut oversized = STANDARD.decode(sample_png(2, 1)).unwrap();
        oversized[16..20].copy_from_slice(&2049u32.to_be_bytes());
        assert!(prepare_sticker(&STANDARD.encode(oversized), "test").is_err());
        let mut corrupted = STANDARD.decode(sample_png(2, 1)).unwrap();
        corrupted[29] ^= 1;
        assert!(prepare_sticker(&STANDARD.encode(corrupted), "test").is_err());
        let mut extra = STANDARD.decode(sample_png(2, 1)).unwrap();
        extra.extend_from_slice(b"\0\0\0\0IEND\xaeB`\x82");
        assert!(prepare_sticker(&STANDARD.encode(extra), "test").is_err());
        let mut animated = STANDARD.decode(sample_png(2, 1)).unwrap();
        insert_chunk(&mut animated, b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
        assert!(prepare_sticker(&STANDARD.encode(animated), "test").is_err());
        let mut metadata = STANDARD.decode(sample_png(2, 1)).unwrap();
        insert_chunk(
            &mut metadata,
            b"tEXt",
            b"Comment\0original fixture metadata",
        );
        let stripped = prepare_sticker(&STANDARD.encode(metadata), "test").unwrap();
        let stripped = STANDARD
            .decode(stripped.data_url.strip_prefix(PNG_PREFIX).unwrap())
            .unwrap();
        assert!(!stripped.windows(4).any(|bytes| bytes == b"tEXt"));
        let mut too_many_pixels = STANDARD.decode(sample_png(2, 1)).unwrap();
        too_many_pixels[16..20].copy_from_slice(&2048u32.to_be_bytes());
        too_many_pixels[20..24].copy_from_slice(&2048u32.to_be_bytes());
        assert!(prepare_sticker(&STANDARD.encode(too_many_pixels), "test").is_err());
        assert!(state.without_sticker().validate().is_ok());
    }
}
