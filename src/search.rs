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
    image_path: &Path,
    fixture: &Path,
    input_face: &face::FaceScan,
    min_face_similarity: f32,
    serpapi_key: Option<&str>,
    image_url: Option<&str>,
) -> Result<SearchReport> {
    match provider {
        SearchProvider::Fixture => fixture_search(fixture),
        SearchProvider::Serpapi => serpapi_search(
            serpapi_key,
            image_url,
            image_path,
            input_face,
            min_face_similarity,
        ),
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
    image_path: &Path,
    input_face: &face::FaceScan,
    min_face_similarity: f32,
) -> Result<SearchReport> {
    let api_key = api_key.context("SERPAPI_KEY or --serpapi-key is required")?;
    let query = prepare_serpapi_query(api_key, image_url, image_path)?;
    let url = query.google_lens_url(api_key);

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

enum SerpApiLensQuery {
    ImageId(String),
    Url(String),
}

impl SerpApiLensQuery {
    fn google_lens_url(&self, api_key: &str) -> String {
        match self {
            Self::ImageId(image_id) => format!(
                "https://serpapi.com/search.json?engine=google_lens&image_id={}&api_key={}",
                urlencoding::encode(image_id),
                urlencoding::encode(api_key)
            ),
            Self::Url(image_url) => format!(
                "https://serpapi.com/search.json?engine=google_lens&url={}&api_key={}",
                urlencoding::encode(image_url),
                urlencoding::encode(api_key)
            ),
        }
    }
}

fn prepare_serpapi_query(
    api_key: &str,
    image_url: Option<&str>,
    image_path: &Path,
) -> Result<SerpApiLensQuery> {
    if let Some(image_url) = image_url {
        return Ok(SerpApiLensQuery::Url(image_url.to_string()));
    }

    let metadata = std::fs::metadata(image_path)
        .with_context(|| format!("checking image size for {}", image_path.display()))?;
    if metadata.len() > 500_000 {
        bail!(
            "SerpAPI image upload accepts images up to 500 KB; {} is {} KB. Compress it or pass --image-url for a public image.",
            image_path.display(),
            metadata.len() / 1024
        );
    }

    let form_image = format!("image=@{}", image_path.display());
    let output = Command::new("curl")
        .args([
            "-sS",
            "--fail",
            "--max-time",
            "30",
            "-X",
            "POST",
            "https://serpapi.com/image",
            "-F",
            &form_image,
            "-F",
            &format!("api_key={api_key}"),
        ])
        .output()
        .context("uploading image to SerpAPI Image API through curl")?;
    if !output.status.success() {
        bail!(
            "SerpAPI image upload failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let response: SerpApiImageUploadResponse =
        serde_json::from_slice(&output.stdout).context("decoding SerpAPI image upload response")?;
    if let Some(error) = response.error.or(response.message) {
        bail!("SerpAPI image upload failed: {error}");
    }
    let image_id = response
        .image_id
        .context("SerpAPI image upload response did not include image_id")?;
    Ok(SerpApiLensQuery::ImageId(image_id))
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
struct SerpApiImageUploadResponse {
    image_id: Option<String>,
    error: Option<String>,
    message: Option<String>,
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
