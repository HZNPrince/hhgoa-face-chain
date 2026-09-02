use crate::{face::FaceScan, search::SearchHit};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub schema: String,
    pub created_at: DateTime<Utc>,
    pub face_image_sha256: String,
    pub face_encoding: String,
    pub discovered_title: String,
    pub discovered_url: String,
    pub discovered_source: String,
    pub discovered_snippet: Option<String>,
    pub discovered_image_url: Option<String>,
}

impl EvidenceRecord {
    pub fn new(face: &FaceScan, hit: &SearchHit) -> Self {
        Self {
            schema: "hhgoa-face-chain/evidence-v1".to_string(),
            created_at: Utc::now(),
            face_image_sha256: face.image_sha256.clone(),
            face_encoding: face.encoding.clone(),
            discovered_title: hit.title.clone(),
            discovered_url: hit.url.clone(),
            discovered_source: hit.source.clone(),
            discovered_snippet: hit.snippet.clone(),
            discovered_image_url: hit.image_url.clone(),
        }
    }

    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).expect("EvidenceRecord must serialize")
    }

    pub fn hash_hex(&self) -> String {
        hex::encode(Sha256::digest(self.canonical_json().as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        face::{FaceBox, FaceScan},
        search::SearchHit,
    };

    #[test]
    fn evidence_hash_is_stable_for_same_record() {
        let face = FaceScan {
            image_path: "samples/input.jpg".to_string(),
            image_sha256: "abc".to_string(),
            detector: "test".to_string(),
            bbox: FaceBox {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            confidence: 0.9,
            encoding: "phash256:def".to_string(),
        };
        let hit = SearchHit {
            title: "Post".to_string(),
            url: "https://example.com/post".to_string(),
            source: "test".to_string(),
            snippet: Some("hello".to_string()),
            image_url: None,
        };
        let one = EvidenceRecord::new(&face, &hit);
        let two = one.clone();

        assert_eq!(one.hash_hex(), two.hash_hex());
    }
}
