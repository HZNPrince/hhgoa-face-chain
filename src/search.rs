use crate::cli::SearchProvider;
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchReport {
    pub selected: SearchHit,
    pub candidates: Vec<SearchHit>,
}

pub fn find_match(
    provider: &SearchProvider,
    fixture: &Path,
    serpapi_key: Option<&str>,
    image_url: Option<&str>,
) -> Result<SearchReport> {
    match provider {
        SearchProvider::Fixture => fixture_search(fixture),
        SearchProvider::Serpapi => serpapi_search(serpapi_key, image_url),
    }
}

fn fixture_search(path: &Path) -> Result<SearchReport> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("reading fixture {}", path.display()))?;
    let selected: SearchHit = serde_json::from_str(&json).context("parsing fixture search hit")?;
    Ok(SearchReport {
        selected: selected.clone(),
        candidates: vec![selected],
    })
}

fn serpapi_search(api_key: Option<&str>, image_url: Option<&str>) -> Result<SearchReport> {
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

    let candidates = response
        .visual_matches
        .into_iter()
        .filter_map(SerpApiVisualMatch::into_search_hit)
        .collect::<Vec<_>>();

    let selected = candidates
        .iter()
        .find(|hit| is_social_url(&hit.url))
        .or_else(|| candidates.first())
        .cloned()
        .context("SerpAPI returned no visual match with a link")?;

    Ok(SearchReport {
        selected,
        candidates: candidates.into_iter().take(10).collect(),
    })
}

fn is_social_url(url: &str) -> bool {
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
            image_url: self.thumbnail,
        })
    }
}
