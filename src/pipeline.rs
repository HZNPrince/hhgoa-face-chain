use crate::{chain, cli::Cli, evidence::EvidenceRecord, face, search};
use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct PipelineReport {
    face: face::FaceScan,
    search: search::SearchReport,
    evidence_hash: String,
    chain_receipt: chain::ChainReceipt,
}

pub fn run(cli: Cli) -> Result<()> {
    println!("1/4 scanning face: {}", cli.image.display());
    let face = face::scan_face(&cli.image)?;
    println!(
        "    face bbox: x={} y={} w={} h={}",
        face.bbox.x, face.bbox.y, face.bbox.width, face.bbox.height
    );
    println!("    face encoding: {}", face.encoding);

    println!(
        "2/4 searching web/social source with {:?}",
        cli.search_provider
    );
    let search_report = search::find_match(
        &cli.search_provider,
        &cli.fixture,
        &face,
        cli.min_face_similarity,
        cli.serpapi_key.as_deref(),
        cli.image_url.as_deref(),
    )?;
    if !search_report.candidates.is_empty() {
        println!("    verified candidate scores:");
        for (index, candidate) in search_report.candidates.iter().take(12).enumerate() {
            println!(
                "      {}. {:.3} {} {} ({})",
                index + 1,
                candidate.face_similarity.unwrap_or(0.0),
                if candidate.face_verified {
                    "PASS"
                } else {
                    "FAIL"
                },
                candidate.title,
                candidate.url
            );
        }
        let social_candidates = search_report
            .candidates
            .iter()
            .filter(|candidate| search::is_social_url(&candidate.url))
            .take(8)
            .collect::<Vec<_>>();
        if !social_candidates.is_empty() {
            println!("    social candidates seen:");
            for candidate in social_candidates {
                println!(
                    "      {:.3} {} {} ({})",
                    candidate.face_similarity.unwrap_or(0.0),
                    if candidate.face_verified {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    candidate.title,
                    candidate.url
                );
            }
        }
    }
    let Some(selected) = search_report.selected.as_ref() else {
        bail!("no reverse-image candidates passed face verification");
    };
    println!("    selected verified match: {}", selected.title);
    println!("    url: {}", selected.url);
    println!(
        "    face similarity: {:.3}",
        selected.face_similarity.unwrap_or(0.0)
    );

    println!("3/4 creating canonical evidence hash");
    let evidence = EvidenceRecord::new(&face, selected);
    let evidence_hash = evidence.hash_hex();
    std::fs::create_dir_all("data").context("creating data directory")?;
    std::fs::write(
        "data/evidence.json",
        serde_json::to_string_pretty(&evidence)?,
    )
    .context("writing data/evidence.json")?;
    println!("    sha256: {evidence_hash}");

    println!("4/4 publishing and verifying on {:?}", cli.chain_provider);
    let receipt = chain::publish_and_verify(
        &cli.chain_provider,
        &evidence_hash,
        &cli.local_chain,
        &cli.solana_cluster,
        cli.skip_verify,
    )?;
    println!("    record: {}", receipt.record_id);
    println!("    verification: {}", receipt.verification);

    let report = PipelineReport {
        face,
        search: search_report,
        evidence_hash,
        chain_receipt: receipt,
    };
    std::fs::write("data/report.json", serde_json::to_string_pretty(&report)?)
        .context("writing data/report.json")?;

    println!();
    println!("DONE: face scan -> discovered post -> chain verification complete");
    println!("Report written to data/report.json");
    Ok(())
}
