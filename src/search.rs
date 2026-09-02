use crate::{cli::SearchProvider, face};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub source: String,
    pub snippet: Option<String>,
    pub image_url: Option<String>,
    pub verification_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_similarity: Option<f32>,
    #[serde(default)]
    pub face_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub selected: Option<SearchHit>,
    pub candidates: Vec<SearchHit>,
}

pub fn find_match(
    provider: &SearchProvider,
    fixture: &Path,
    input_face: &face::FaceScan,
    min_face_similarity: f32,
    serpapi_key: Option<&str>,
    image_url: Option<&str>,
) -> Result<SearchReport> {
    match provider {
        SearchProvider::Fixture => fixture_search(fixture),
        SearchProvider::Serpapi => {
            serpapi_search(serpapi_key, image_url, input_face, min_face_similarity)
        }
    }
}

fn fixture_search(path: &Path) -> Result<SearchReport> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("reading fixture {}", path.display()))?;
    let selected: SearchHit = serde_json::from_str(&json).context("parsing fixture search hit")?;
    let selected = SearchHit {
        face_similarity: Some(1.0),
        face_verified: true,
        ..selected
    };
    Ok(SearchReport {
        selected: Some(selected.clone()),
        candidates: vec![selected],
    })
}

fn serpapi_search(
    api_key: Option<&str>,
    image_url: Option<&str>,
    input_face: &face::FaceScan,
    min_face_similarity: f32,
) -> Result<SearchReport> {
    let api_key = api_key.context("SERPAPI_KEY or --serpapi-key is required")?;
    let image_url = image_url.context("--image-url is required for SerpAPI Google Lens")?;
    let url = format!(
        "https://serpapi.com/search.json?engine=google_lens&url={}&api_key={}",
        urlencoding::encode(image_url),
        urlencoding::encode(api_key)
    );

    let output = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "30", &url])
        .output()
        .context("calling SerpAPI Google Lens through curl")?;
    if !output.status.success() {
        bail!(
            "SerpAPI curl request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let response: SerpApiResponse =
        serde_json::from_slice(&output.stdout).context("decoding SerpAPI response")?;

    let mut candidates = response
        .visual_matches
        .into_iter()
        .filter_map(SerpApiVisualMatch::into_search_hit)
        .take(30)
        .map(|candidate| verify_candidate_face(candidate, input_face, min_face_similarity))
        .collect::<Vec<_>>();

    let selected = candidates
        .iter()
        .find(|hit| hit.face_verified && is_social_url(&hit.url))
        .or_else(|| candidates.iter().find(|hit| hit.face_verified))
        .cloned();

    candidates.sort_by(|a, b| {
        b.face_similarity
            .unwrap_or(0.0)
            .total_cmp(&a.face_similarity.unwrap_or(0.0))
    });

    Ok(SearchReport {
        selected,
        candidates,
    })
}

fn verify_candidate_face(
    mut candidate: SearchHit,
    input_face: &face::FaceScan,
    min_face_similarity: f32,
) -> SearchHit {
    let image_url = candidate
        .verification_image_url
        .as_deref()
        .or(candidate.image_url.as_deref());

    let Some(image_url) = image_url else {
        return candidate;
    };

    let Ok(bytes) = download_bytes(image_url) else {
        return candidate;
    };

    let Ok(candidate_face) = face::scan_face_bytes(image_url.to_string(), &bytes) else {
        return candidate;
    };

    let similarity = face::similarity(input_face, &candidate_face);
    candidate.face_similarity = Some(similarity);
    candidate.face_verified = similarity >= min_face_similarity;
    candidate
}

fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let output = Command::new("curl")
        .args(["-L", "-sS", "--fail", "--max-time", "5", url])
        .output()
        .context("downloading candidate thumbnail")?;
    if !output.status.success() {
        bail!(
            "candidate thumbnail download failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

pub fn is_social_url(url: &str) -> bool {
    [
        "instagram.com",
        "x.com",
        "twitter.com",
        "linkedin.com",
        "facebook.com",
        "threads.net",
        "tiktok.com",
        "youtube.com",
    ]
    .iter()
    .any(|domain| url.contains(domain))
}

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    #[serde(default)]
    visual_matches: Vec<SerpApiVisualMatch>,
}

#[derive(Debug, Deserialize)]
struct SerpApiVisualMatch {
    title: Option<String>,
    link: Option<String>,
    source: Option<String>,
    thumbnail: Option<String>,
    image: Option<String>,
}

impl SerpApiVisualMatch {
    fn into_search_hit(self) -> Option<SearchHit> {
        let url = self.link?;
        if url.trim().is_empty() {
            return None;
        }

        Some(SearchHit {
            title: self
                .title
                .unwrap_or_else(|| "Untitled visual match".to_string()),
            url,
            source: self
                .source
                .unwrap_or_else(|| "SerpAPI Google Lens".to_string()),
            snippet: None,
            verification_image_url: self.image,
            image_url: self.thumbnail,
            face_similarity: None,
            face_verified: false,
        })
    }
}
