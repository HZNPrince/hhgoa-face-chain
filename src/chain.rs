use crate::cli::ChainProvider;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};

#[derive(Debug, Clone, Serialize)]
pub struct ChainReceipt {
    pub provider: String,
    pub record_id: String,
    pub evidence_hash: String,
    pub verification: String,
}

pub fn publish_and_verify(
    provider: &ChainProvider,
    evidence_hash: &str,
    local_chain_path: &Path,
    solana_cluster: &str,
    skip_verify: bool,
) -> Result<ChainReceipt> {
    match provider {
        ChainProvider::Local => local_publish_and_verify(evidence_hash, local_chain_path),
        ChainProvider::SolanaMemo => {
            solana_memo_publish_and_verify(evidence_hash, solana_cluster, skip_verify)
        }
    }
}

fn local_publish_and_verify(evidence_hash: &str, path: &Path) -> Result<ChainReceipt> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut chain = if path.exists() {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str::<Vec<LocalBlock>>(&raw).context("parsing local chain")?
    } else {
        Vec::new()
    };

    let previous_hash = chain
        .last()
        .map(|block| block.block_hash.clone())
        .unwrap_or_else(|| "GENESIS".to_string());
    let height = chain.len() as u64;
    let block = LocalBlock::new(height, previous_hash, evidence_hash.to_string());
    let block_hash = block.block_hash.clone();
    chain.push(block);

    let pretty = serde_json::to_string_pretty(&chain).context("serializing local chain")?;
    std::fs::write(path, pretty).with_context(|| format!("writing {}", path.display()))?;

    let verified = chain.iter().any(|block| {
        block.evidence_hash == evidence_hash && block.block_hash == block.compute_hash()
    });
    if !verified {
        bail!("local chain verification failed");
    }

    Ok(ChainReceipt {
        provider: "local-simulated-blockchain".to_string(),
        record_id: format!("{}#{}", path.display(), height),
        evidence_hash: evidence_hash.to_string(),
        verification: format!("PASS block_hash={block_hash}"),
    })
}

fn solana_memo_publish_and_verify(
    evidence_hash: &str,
    cluster: &str,
    skip_verify: bool,
) -> Result<ChainReceipt> {
    let address = run_solana(&["address"]).context("reading Solana wallet address")?;
    let memo = format!("hhgoa-face-chain:{evidence_hash}");
    let signature_output = run_solana(&[
        "-u",
        cluster,
        "transfer",
        address.trim(),
        "0.000001",
        "--allow-unfunded-recipient",
        "--with-memo",
        &memo,
    ])
    .context("submitting Solana memo transaction")?;

    let signature = signature_output
        .lines()
        .find(|line| line.trim().len() > 40)
        .unwrap_or(signature_output.trim())
        .trim()
        .to_string();

    if skip_verify {
        return Ok(ChainReceipt {
            provider: format!("solana-memo-{cluster}"),
            record_id: signature,
            evidence_hash: evidence_hash.to_string(),
            verification: "SKIPPED".to_string(),
        });
    }

    let confirmed = run_solana(&["-u", cluster, "confirm", "-v", &signature])
        .context("fetching Solana transaction for verification")?;
    if !confirmed.contains(&memo) {
        bail!("Solana transaction was found, but expected memo was not present");
    }

    Ok(ChainReceipt {
        provider: format!("solana-memo-{cluster}"),
        record_id: signature,
        evidence_hash: evidence_hash.to_string(),
        verification: "PASS memo found in confirmed transaction".to_string(),
    })
}

fn run_solana(args: &[&str]) -> Result<String> {
    let output = Command::new("solana")
        .args(args)
        .output()
        .context("running solana CLI")?;

    if !output.status.success() {
        bail!(
            "solana command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalBlock {
    height: u64,
    created_at: DateTime<Utc>,
    previous_hash: String,
    evidence_hash: String,
    block_hash: String,
}

impl LocalBlock {
    fn new(height: u64, previous_hash: String, evidence_hash: String) -> Self {
        let mut block = Self {
            height,
            created_at: Utc::now(),
            previous_hash,
            evidence_hash,
            block_hash: String::new(),
        };
        block.block_hash = block.compute_hash();
        block
    }

    fn compute_hash(&self) -> String {
        let payload = format!(
            "{}|{}|{}|{}",
            self.height,
            self.created_at.to_rfc3339(),
            self.previous_hash,
            self.evidence_hash
        );
        hex::encode(Sha256::digest(payload.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_chain_publishes_and_verifies() {
        let path = std::env::temp_dir().join("hhgoa-face-chain-local-test.json");
        let _ = std::fs::remove_file(&path);

        let receipt = local_publish_and_verify("abc123", &path).expect("publish local block");
        assert_eq!(receipt.provider, "local-simulated-blockchain");
        assert!(receipt.verification.starts_with("PASS"));

        let chain = std::fs::read_to_string(&path).expect("read chain");
        assert!(chain.contains("abc123"));

        let _ = std::fs::remove_file(path);
    }
}
