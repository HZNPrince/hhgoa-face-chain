use anyhow::{Context, Result, bail};
use image::{DynamicImage, GenericImageView, Pixel, imageops::FilterType};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct FaceScan {
    pub image_path: String,
    pub image_sha256: String,
    pub detector: String,
    pub bbox: FaceBox,
    pub confidence: f32,
    pub encoding: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FaceBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn scan_face(path: &Path) -> Result<FaceScan> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let image_sha256 = hex::encode(Sha256::digest(&bytes));
    let image = image::load_from_memory(&bytes).context("decoding input image")?;
    let bbox = detect_skin_tone_face_candidate(&image)?;
    let encoding = encode_face_crop(&image, &bbox);

    Ok(FaceScan {
        image_path: path.display().to_string(),
        image_sha256,
        detector: "local-rust-skin-tone-candidate-v1".to_string(),
        bbox,
        confidence: 0.64,
        encoding,
    })
}

fn detect_skin_tone_face_candidate(image: &DynamicImage) -> Result<FaceBox> {
    let (w, h) = image.dimensions();
    if w < 32 || h < 32 {
        bail!("image is too small for face scanning");
    }

    let rgb = image.to_rgb8();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut hits = 0u32;

    for (x, y, pixel) in rgb.enumerate_pixels() {
        let [r, g, b] = pixel.0;
        if is_skin_tone(r, g, b) && is_in_portrait_region(x, y, w, h) {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            hits += 1;
        }
    }

    let min_hits = (w * h / 300).max(40);
    if hits < min_hits {
        bail!("no likely face region detected; use a clear, front-facing portrait");
    }

    let raw_width = max_x.saturating_sub(min_x).max(1);
    let raw_height = max_y.saturating_sub(min_y).max(1);
    let pad_x = raw_width / 3;
    let pad_y = raw_height / 2;
    let x = min_x.saturating_sub(pad_x);
    let y = min_y.saturating_sub(pad_y);
    let width = (raw_width + pad_x * 2).min(w - x);
    let height = (raw_height + pad_y * 2).min(h - y);

    Ok(FaceBox {
        x,
        y,
        width,
        height,
    })
}

fn is_in_portrait_region(x: u32, y: u32, w: u32, h: u32) -> bool {
    let left = w / 10;
    let right = w - left;
    let top = h / 20;
    let bottom = h - top;
    x >= left && x <= right && y >= top && y <= bottom
}

fn is_skin_tone(r: u8, g: u8, b: u8) -> bool {
    let r = r as i16;
    let g = g as i16;
    let b = b as i16;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    r > 95 && g > 40 && b > 20 && max - min > 15 && (r - g).abs() > 15 && r > g && r > b
}

fn encode_face_crop(image: &DynamicImage, bbox: &FaceBox) -> String {
    let crop = image.crop_imm(bbox.x, bbox.y, bbox.width, bbox.height);
    let gray = crop.grayscale().resize_exact(16, 16, FilterType::Triangle);
    let luma = gray.to_luma8();
    let avg = luma.pixels().map(|p| p.channels()[0] as u32).sum::<u32>() / 256;
    let mut bits = String::with_capacity(256);

    for p in luma.pixels() {
        bits.push(if p.channels()[0] as u32 >= avg {
            '1'
        } else {
            '0'
        });
    }

    let digest = Sha256::digest(bits.as_bytes());
    format!("phash256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn scans_synthetic_portrait() {
        let mut image = RgbImage::from_pixel(160, 160, Rgb([28, 34, 44]));
        for x in 50..110 {
            for y in 35..115 {
                image.put_pixel(x, y, Rgb([190, 128, 92]));
            }
        }

        let path = std::env::temp_dir().join("hhgoa-face-chain-synthetic.png");
        image.save(&path).expect("save synthetic image");

        let scan = scan_face(&path).expect("scan face");
        assert!(scan.bbox.width > 40);
        assert!(scan.bbox.height > 60);
        assert!(scan.encoding.starts_with("phash256:"));

        let _ = std::fs::remove_file(path);
    }
}
