use crate::{chain, cli::Cli, evidence::EvidenceRecord, face, search};
use anyhow::{Context, Result, bail};
use serde::Serialize;

const EVIDENCE_PATH: &str = "data/evidence.json";
const REPORT_PATH: &str = "data/report.json";

#[derive(Debug, Serialize)]
struct PipelineReport {
    face: face::FaceScan,
    search: search::SearchReport,
    evidence_hash: String,
    chain_receipt: chain::ChainReceipt,
}

pub fn run(cli: Cli) -> Result<()> {
    print_banner();

    print_step(1, "Face scan");
    println!("    Input image    {}", cli.image.display());
    let face = face::scan_face(&cli.image)?;
    println!("    Detector       {}", face.detector);
    println!("    Face box       {}", format_bbox(&face.bbox));
    println!("    Encoding       {}", short_hash(&face.encoding, 28));

    print_step(2, "Web/social discovery");
    println!("    Provider       {:?}", cli.search_provider);
    println!("    Face threshold {:.2}", cli.min_face_similarity);
    if let Some(image_url) = &cli.image_url {
        println!("    Search input   public URL ({})", clip(image_url, 70));
    } else if matches!(cli.search_provider, crate::cli::SearchProvider::Serpapi) {
        println!("    Search input   local image upload");
    }
    let search_report = search::find_match(
        &cli.search_provider,
        &cli.image,
        &cli.fixture,
        &face,
        cli.min_face_similarity,
        cli.serpapi_key.as_deref(),
        cli.image_url.as_deref(),
    )?;
    print_candidates("Top candidates", search_report.candidates.iter().take(10));
    print_candidates(
        "Social candidates",
        search_report
            .candidates
            .iter()
            .filter(|candidate| search::is_social_url(&candidate.url))
            .take(8),
    );

    let Some(selected) = search_report.selected.as_ref() else {
        bail!("no reverse-image candidates were discovered");
    };

    print_selected(selected);

    print_step(3, "Evidence fingerprint");
    let evidence = EvidenceRecord::new(&face, selected);
    let evidence_hash = evidence.hash_hex();
    std::fs::create_dir_all("data").context("creating data directory")?;
    std::fs::write(EVIDENCE_PATH, serde_json::to_string_pretty(&evidence)?)
        .context("writing data/evidence.json")?;
    println!("    SHA-256        {evidence_hash}");
    println!("    Evidence file  {EVIDENCE_PATH}");

    print_step(4, "Blockchain record");
    println!("    Chain          {:?}", cli.chain_provider);
    let receipt = chain::publish_and_verify(
        &cli.chain_provider,
        &evidence_hash,
        &cli.local_chain,
        &cli.solana_cluster,
        cli.skip_verify,
    )?;
    println!("    Record         {}", receipt.record_id);
    println!("    Verification   {}", receipt.verification);

    let report = PipelineReport {
        face,
        search: search_report,
        evidence_hash,
        chain_receipt: receipt,
    };
    std::fs::write(REPORT_PATH, serde_json::to_string_pretty(&report)?)
        .context("writing data/report.json")?;

    println!();
    println!("Result");
    println!("  Status          COMPLETE");
    println!("  Pipeline        face scan -> discovery -> evidence hash -> chain verification");
    println!("  Report file     {REPORT_PATH}");
    Ok(())
}

fn print_banner() {
    println!();
    println!("HH Goa Face Chain");
    println!("Face scan -> web/social discovery -> blockchain verification");
    println!("{}", "-".repeat(72));
}

fn print_step(number: u8, title: &str) {
    println!();
    println!("[{number}/4] {title}");
}

fn print_candidates<'a>(title: &str, candidates: impl Iterator<Item = &'a search::SearchHit>) {
    let candidates = candidates.collect::<Vec<_>>();
    if candidates.is_empty() {
        return;
    }

    println!();
    println!("    {title}");
    println!("    {:<3} {:<10} {:<7} {}", "#", "Check", "Score", "Result");
    println!("    {}", "-".repeat(92));
    for (index, candidate) in candidates.iter().enumerate() {
        println!(
            "    {:<3} {:<10} {:<7} {}",
            index + 1,
            candidate.face_check_status.label(),
            format_score(candidate.face_similarity),
            clip(&format!("{} - {}", candidate.title, candidate.url), 72)
        );
    }
}

fn print_selected(candidate: &search::SearchHit) {
    println!();
    println!("    Selected match");
    println!("    Title          {}", clip(&candidate.title, 72));
    println!("    URL            {}", candidate.url);
    println!("    Source         {}", candidate.source);
    println!("    Face check     {}", candidate.face_check_status.label());
    if let Some(similarity) = candidate.face_similarity {
        println!("    Similarity     {similarity:.3}");
    }
}

fn format_bbox(bbox: &face::FaceBox) -> String {
    format!(
        "x={} y={} width={} height={}",
        bbox.x, bbox.y, bbox.width, bbox.height
    )
}

fn format_score(score: Option<f32>) -> String {
    score
        .map(|score| format!("{score:.3}"))
        .unwrap_or_else(|| "-".to_string())
}

fn short_hash(value: &str, keep: usize) -> String {
    if value.len() <= keep {
        return value.to_string();
    }
    format!("{}...", &value[..keep])
}

fn clip(value: &str, max_chars: usize) -> String {
    let mut clipped = value.chars().take(max_chars).collect::<String>();
    if clipped.len() < value.len() {
        clipped.push_str("...");
    }
    clipped
}
