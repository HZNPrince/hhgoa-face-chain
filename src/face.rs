use anyhow::{Context, Result, bail};
use image::{DynamicImage, GenericImageView, Pixel, imageops::FilterType};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, path::Path};

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

    let scan_width = w.min(360);
    let scan_height = ((h as f32) * (scan_width as f32 / w as f32)).round() as u32;
    let scan = image
        .resize_exact(scan_width, scan_height.max(32), FilterType::Triangle)
        .to_rgb8();
    let skin = scan
        .pixels()
        .map(|pixel| {
            let [r, g, b] = pixel.0;
            is_skin_tone(r, g, b)
        })
        .collect::<Vec<_>>();
    let components = skin_components(&skin, scan_width, scan_height.max(32));
    let best = components
        .into_iter()
        .filter(|component| component.area >= 80)
        .filter(|component| component.width() >= 8 && component.height() >= 8)
        .filter(|component| {
            let aspect = component.width() as f32 / component.height() as f32;
            (0.45..=1.45).contains(&aspect)
        })
        .max_by_key(|component| component.score(scan_width, scan_height.max(32)));

    let Some(component) = best else {
        bail!("no likely face region detected; use a clear, front-facing portrait");
    };

    let scale_x = w as f32 / scan_width as f32;
    let scale_y = h as f32 / scan_height.max(32) as f32;
    let min_x = (component.min_x as f32 * scale_x).round() as u32;
    let min_y = (component.min_y as f32 * scale_y).round() as u32;
    let max_x = (component.max_x as f32 * scale_x).round() as u32;
    let max_y = (component.max_y as f32 * scale_y).round() as u32;

    let raw_width = max_x.saturating_sub(min_x).max(1);
    let raw_height = max_y.saturating_sub(min_y).max(1);
    let pad_x = raw_width / 2;
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

fn is_skin_tone(r: u8, g: u8, b: u8) -> bool {
    let r = r as i16;
    let g = g as i16;
    let b = b as i16;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    r > 95 && g > 40 && b > 20 && max - min > 15 && (r - g).abs() > 15 && r > g && r > b
}

fn skin_components(skin: &[bool], width: u32, height: u32) -> Vec<SkinComponent> {
    let mut visited = vec![false; skin.len()];
    let mut components = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if !skin[idx] || visited[idx] {
                continue;
            }

            let mut queue = VecDeque::from([(x, y)]);
            let mut component = SkinComponent::new(x, y);
            visited[idx] = true;

            while let Some((cx, cy)) = queue.pop_front() {
                component.include(cx, cy);
                for (nx, ny) in neighbors(cx, cy, width, height) {
                    let nidx = (ny * width + nx) as usize;
                    if skin[nidx] && !visited[nidx] {
                        visited[nidx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }

            components.push(component);
        }
    }

    components
}

fn neighbors(x: u32, y: u32, width: u32, height: u32) -> impl Iterator<Item = (u32, u32)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push((x - 1, y));
    }
    if y > 0 {
        out.push((x, y - 1));
    }
    if x + 1 < width {
        out.push((x + 1, y));
    }
    if y + 1 < height {
        out.push((x, y + 1));
    }
    out.into_iter()
}

#[derive(Debug)]
struct SkinComponent {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    area: u32,
}

impl SkinComponent {
    fn new(x: u32, y: u32) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
            area: 0,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
        self.area += 1;
    }

    fn width(&self) -> u32 {
        self.max_x.saturating_sub(self.min_x).max(1)
    }

    fn height(&self) -> u32 {
        self.max_y.saturating_sub(self.min_y).max(1)
    }

    fn score(&self, image_width: u32, image_height: u32) -> u32 {
        let center_x = (self.min_x + self.max_x) / 2;
        let center_y = (self.min_y + self.max_y) / 2;
        let horizontal_bonus = image_width.saturating_sub(center_x.abs_diff(image_width / 2));
        let upper_half_bonus = image_height.saturating_sub(center_y);
        self.area + horizontal_bonus + upper_half_bonus / 2
    }
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

        let path = std::env::temp_dir().join("hhgoa-face-chain-synthetic.jpg");
        image.save(&path).expect("save synthetic image");

        let scan = scan_face(&path).expect("scan face");
        assert!(scan.bbox.width > 40);
        assert!(scan.bbox.height > 60);
        assert!(scan.encoding.starts_with("phash256:"));

        let _ = std::fs::remove_file(path);
    }
}
