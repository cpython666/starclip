//! Bounded, local PNG preparation. No clipboard or filesystem access happens here.
use crate::store::Result;
use image::{
    codecs::png::{PngDecoder, PngEncoder},
    DynamicImage, ImageBuffer, ImageDecoder, ImageEncoder, Limits, Rgba,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};

pub const MAX_PIXELS: u64 = 16_000_000;
pub const MAX_PNG_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_RGBA_BYTES: usize = 64 * 1024 * 1024;
const MAX_EDGE: u32 = 16_000;
const THUMBNAIL_EDGE: u32 = 320;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

pub struct PreparedImage {
    pub(crate) png: Vec<u8>,
    pub(crate) thumbnail: Vec<u8>,
    metadata: ImageMetadata,
    digest: String,
}
impl PreparedImage {
    pub fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn rgba_length(width: u32, height: u32) -> Result<usize> {
    let pixels = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || pixels > MAX_PIXELS
        || pixels * 4 > MAX_RGBA_BYTES as u64
    {
        return Err("图片尺寸超过限制：最多 1600 万像素、边长 16000，解码数据最多 64 MiB。".into());
    }
    Ok((pixels * 4) as usize)
}

struct PngBuffer(Vec<u8>);
impl Write for PngBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_PNG_BYTES {
            return Err(std::io::Error::other("PNG 图片不能超过 20 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut output = PngBuffer(Vec::new());
    PngEncoder::new(&mut output)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| format!("无法编码 PNG 图片：{error}"))?;
    Ok(output.0)
}

pub fn fingerprint_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    if rgba.len() != rgba_length(width, height)? {
        return Err("图片像素长度与尺寸不符。".into());
    }
    let mut hash = Sha256::new();
    hash.update(width.to_le_bytes());
    hash.update(height.to_le_bytes());
    hash.update(rgba);
    Ok(format!("sha256:{:x}", hash.finalize()))
}

pub fn prepare_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<PreparedImage> {
    let digest = fingerprint_rgba(width, height, rgba)?;
    let png = encode_rgba(width, height, rgba)?;
    let source = ImageBuffer::<Rgba<u8>, &[u8]>::from_raw(width, height, rgba)
        .ok_or("图片像素长度与尺寸不符。")?;
    let thumbnail = if width <= THUMBNAIL_EDGE && height <= THUMBNAIL_EDGE {
        png.clone()
    } else {
        let longest = width.max(height);
        let thumbnail = image::imageops::thumbnail(
            &source,
            (width * THUMBNAIL_EDGE / longest).max(1),
            (height * THUMBNAIL_EDGE / longest).max(1),
        );
        encode_rgba(thumbnail.width(), thumbnail.height(), thumbnail.as_raw())?
    };
    Ok(PreparedImage {
        metadata: ImageMetadata {
            width,
            height,
            bytes: png.len() as u64,
        },
        png,
        thumbnail,
        digest,
    })
}

pub fn decode_png(png: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    if png.len() > MAX_PNG_BYTES
        || png.len() < 45
        || !png.starts_with(b"\x89PNG\r\n\x1a\n")
        || &png[8..16] != b"\0\0\0\rIHDR"
        || !png.ends_with(b"\0\0\0\0IEND\xaeB`\x82")
    {
        return Err("图片不是完整的 PNG，或超过 20 MiB。".into());
    }
    let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
    rgba_length(width, height)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_RGBA_BYTES as u64);
    let decoder = PngDecoder::with_limits(Cursor::new(png), limits)
        .map_err(|error| format!("PNG 图片损坏或超出解码限制：{error}"))?;
    if decoder.total_bytes() > MAX_RGBA_BYTES as u64
        || decoder.is_apng().map_err(|e| e.to_string())?
    {
        return Err("仅支持解码大小不超过 64 MiB 的静态 PNG 图片。".into());
    }
    let rgba = DynamicImage::from_decoder(decoder)
        .map_err(|error| format!("PNG 图片损坏：{error}"))?
        .into_rgba8()
        .into_raw();
    if rgba.len() != rgba_length(width, height)? {
        return Err("PNG 图片尺寸与像素不符。".into());
    }
    Ok((width, height, rgba))
}

pub fn prepare_png(png: &[u8]) -> Result<PreparedImage> {
    let (width, height, rgba) = decode_png(png)?;
    prepare_rgba(width, height, &rgba)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_roundtrip_thumbnail_identity_and_corruption_limits() {
        let rgba: Vec<u8> = (0..640 * 160)
            .flat_map(|i| [(i % 251) as u8, 110, 230, if i % 2 == 0 { 255 } else { 0 }])
            .collect();
        let prepared = prepare_rgba(640, 160, &rgba).unwrap();
        let (width, height, decoded) = decode_png(&prepared.png).unwrap();
        assert_eq!((width, height), (640, 160));
        assert_eq!(decoded, rgba);
        assert_eq!(
            prepare_png(&prepared.png).unwrap().digest(),
            prepared.digest()
        );
        let (width, height, _) = decode_png(&prepared.thumbnail).unwrap();
        assert_eq!((width, height), (320, 80));
        assert_ne!(
            prepare_rgba(320, 320, &rgba).unwrap().digest(),
            prepared.digest()
        );
        assert!(prepare_rgba(0, 1, &[]).is_err());
        assert!(prepare_rgba(16_001, 1, &[]).is_err());
        assert!(prepare_rgba(4001, 4000, &[]).is_err());
        assert!(prepare_rgba(1, 1, &[0, 0, 0]).is_err());
        assert!(decode_png(&prepared.png[..prepared.png.len() - 1]).is_err());
        let mut damaged = prepared.png.clone();
        damaged[29] ^= 1;
        assert!(decode_png(&damaged).is_err());
        assert!(decode_png(&vec![0; MAX_PNG_BYTES + 1]).is_err());
    }
}
