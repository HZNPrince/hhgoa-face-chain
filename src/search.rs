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

pub fn find_match(
    provider: &SearchProvider,
    fixture: &Path,
    serpapi_key: Option<&str>,
    image_url: Option<&str>,
) -> Result<SearchHit> {
    match provider {
        SearchProvider::Fixture => fixture_search(fixture),
        SearchProvider::Serpapi => serpapi_search(serpapi_key, image_url),
    }
}

fn fixture_search(path: &Path) -> Result<SearchHit> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("reading fixture {}", path.display()))?;
    serde_json::from_str(&json).context("parsing fixture search hit")
}

fn serpapi_search(api_key: Option<&str>, image_url: Option<&str>) -> Result<SearchHit> {
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

    let hit = response
        .visual_matches
        .into_iter()
        .find(|item| item.link.is_some())
        .context("SerpAPI returned no visual match with a link")?;

    let url = hit.link.unwrap();
    if url.trim().is_empty() {
        bail!("SerpAPI returned an empty result URL");
    }

    Ok(SearchHit {
        title: hit
            .title
            .unwrap_or_else(|| "Untitled visual match".to_string()),
        url,
        source: hit
            .source
            .unwrap_or_else(|| "SerpAPI Google Lens".to_string()),
        snippet: None,
        image_url: hit.thumbnail,
    })
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
