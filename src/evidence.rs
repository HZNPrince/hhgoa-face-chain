use crate::{face::FaceScan, search::SearchHit};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_EVIDENCE_CANDIDATES: usize = 8;

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
    pub face_check_status: String,
    pub face_similarity: Option<f32>,
    pub discovered_candidates: Vec<EvidenceCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCandidate {
    pub rank: usize,
    pub title: String,
    pub url: String,
    pub source: String,
    pub is_social: bool,
    pub face_check_status: String,
    pub face_similarity: Option<f32>,
}

impl EvidenceRecord {
    pub fn new<'a>(
        face: &FaceScan,
        primary: &SearchHit,
        candidates: impl IntoIterator<Item = &'a SearchHit>,
    ) -> Self {
        Self {
            schema: "hhgoa-face-chain/evidence-v1".to_string(),
            created_at: Utc::now(),
            face_image_sha256: face.image_sha256.clone(),
            face_encoding: face.encoding.clone(),
            discovered_title: primary.title.clone(),
            discovered_url: primary.url.clone(),
            discovered_source: primary.source.clone(),
            discovered_snippet: primary.snippet.clone(),
            discovered_image_url: primary.image_url.clone(),
            face_check_status: primary.face_check_status.as_str().to_string(),
            face_similarity: primary.face_similarity,
            discovered_candidates: candidates
                .into_iter()
                .take(MAX_EVIDENCE_CANDIDATES)
                .enumerate()
                .map(|(index, candidate)| EvidenceCandidate {
                    rank: index + 1,
                    title: candidate.title.clone(),
                    url: candidate.url.clone(),
                    source: candidate.source.clone(),
                    is_social: crate::search::is_social_url(&candidate.url),
                    face_check_status: candidate.face_check_status.as_str().to_string(),
                    face_similarity: candidate.face_similarity,
                })
                .collect(),
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
        search::{FaceCheckStatus, SearchHit},
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
            signature: "1010".to_string(),
        };
        let hit = SearchHit {
            title: "Post".to_string(),
            url: "https://example.com/post".to_string(),
            source: "test".to_string(),
            snippet: Some("hello".to_string()),
            image_url: None,
            verification_image_url: None,
            face_similarity: Some(1.0),
            face_verified: true,
            face_check_status: FaceCheckStatus::Verified,
        };
        let one = EvidenceRecord::new(&face, &hit, [&hit]);
        let two = one.clone();

        assert_eq!(one.hash_hex(), two.hash_hex());
    }
}
