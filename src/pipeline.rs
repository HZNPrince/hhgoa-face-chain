use crate::{
    chain,
    cli::Cli,
    evidence::{EvidenceRecord, MAX_EVIDENCE_CANDIDATES},
    face, search,
};
use anyhow::{Context, Result, bail};
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;
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
    let evidence_candidates = evidence_shortlist(selected, &search_report.candidates);
    let evidence = EvidenceRecord::new(&face, selected, evidence_candidates);
    let evidence_hash = evidence.hash_hex();
    std::fs::create_dir_all("data").context("creating data directory")?;
    std::fs::write(EVIDENCE_PATH, serde_json::to_string_pretty(&evidence)?)
        .context("writing data/evidence.json")?;
    println!("    SHA-256        {evidence_hash}");
    println!(
        "    Candidates     {} recorded in evidence",
        evidence.discovered_candidates.len()
    );
    println!("    Evidence file  {EVIDENCE_PATH}");

    print_step(4, "Blockchain record");
    println!("    Chain          {:?}", cli.chain_provider);
    if matches!(cli.chain_provider, crate::cli::ChainProvider::SolanaMemo) {
        println!("    Memo           hhgoa-face-chain:{evidence_hash}");
    }
    let receipt = chain::publish_and_verify(
        &cli.chain_provider,
        &evidence_hash,
        &cli.local_chain,
        &cli.solana_cluster,
        cli.skip_verify,
    )?;
    println!("    Record         {}", receipt.record_id);
    println!("    Verification   {}", receipt.verification);
    if let Some(explorer_url) = solana_explorer_url(&receipt) {
        println!(
            "    Explorer       {}",
            clickable_link(&explorer_url, &explorer_url)
        );
    }

    let report = PipelineReport {
        face,
        search: search_report,
        evidence_hash,
        chain_receipt: receipt,
    };
    std::fs::write(REPORT_PATH, serde_json::to_string_pretty(&report)?)
        .context("writing data/report.json")?;

    println!();
    println!("{}", "Result".bold().bright_green());
    println!("  Status          {}", "COMPLETE".green().bold());
    println!("  Pipeline        face scan -> discovery -> evidence hash -> chain verification");
    println!("  Report file     {REPORT_PATH}");
    Ok(())
}

fn print_banner() {
    println!();
    println!("{}", "HH Goa Face Chain".bold().bright_cyan());
    println!(
        "{}",
        "Face scan -> web/social discovery -> blockchain verification".bright_black()
    );
    println!("{}", "─".repeat(72).bright_black());
}

fn print_step(number: u8, title: &str) {
    println!();
    println!(
        "{} {}",
        format!("[{number}/4]").bold().bright_blue(),
        title.bold()
    );
}

fn print_candidates<'a>(title: &str, candidates: impl Iterator<Item = &'a search::SearchHit>) {
    let candidates = candidates.collect::<Vec<_>>();
    if candidates.is_empty() {
        return;
    }

    println!();
    println!("    {}", title.bold().bright_cyan());
    for (index, candidate) in candidates.iter().enumerate() {
        print_candidate(index + 1, candidate);
    }
}

fn print_candidate(index: usize, candidate: &search::SearchHit) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(112);
    table.add_row([
        Cell::new(format!("{index:02}"))
            .fg(Color::Blue)
            .add_attribute(Attribute::Bold),
        Cell::new(format_check(candidate)).fg(check_color(candidate)),
        Cell::new(format!("score {}", format_score(candidate.face_similarity))).fg(Color::Yellow),
        Cell::new(candidate.source.clone()).fg(Color::Magenta),
        Cell::new(clip(&candidate.title, 68)),
    ]);
    println!("{table}");
    println!(
        "    {} {}",
        "Link".bright_black(),
        clickable_link(&candidate.url, &candidate.url)
    );
}

fn print_selected(candidate: &search::SearchHit) {
    println!();
    println!("    {}", "Selected match".bold().bright_green());
    println!("    {}", "─".repeat(72).bright_black());
    println!("    Title          {}", clip(&candidate.title, 92).bold());
    println!(
        "    URL            {}",
        clickable_link(&candidate.url, &candidate.url)
    );
    println!("    Source         {}", candidate.source);
    println!("    Face check     {}", styled_check(candidate));
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

fn format_check(candidate: &search::SearchHit) -> &'static str {
    if candidate.face_verified {
        "VERIFIED"
    } else {
        "DISCOVERED"
    }
}

fn styled_check(candidate: &search::SearchHit) -> String {
    if candidate.face_verified {
        "VERIFIED".green().bold().to_string()
    } else {
        "DISCOVERED".white().to_string()
    }
}

fn check_color(candidate: &search::SearchHit) -> Color {
    if candidate.face_verified {
        Color::Green
    } else {
        Color::White
    }
}

fn clickable_link(label: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
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

fn solana_explorer_url(receipt: &chain::ChainReceipt) -> Option<String> {
    let cluster = receipt.provider.strip_prefix("solana-memo-")?;
    Some(format!(
        "https://explorer.solana.com/tx/{}?cluster={}",
        receipt.record_id, cluster
    ))
}

fn evidence_shortlist<'a>(
    selected: &'a search::SearchHit,
    candidates: &'a [search::SearchHit],
) -> Vec<&'a search::SearchHit> {
    let mut shortlist = Vec::new();
    push_unique(&mut shortlist, selected);

    for candidate in candidates.iter().take(5) {
        push_unique(&mut shortlist, candidate);
    }

    for candidate in candidates
        .iter()
        .filter(|candidate| search::is_social_url(&candidate.url))
    {
        push_unique(&mut shortlist, candidate);
        if shortlist.len() >= MAX_EVIDENCE_CANDIDATES {
            break;
        }
    }

    shortlist.truncate(MAX_EVIDENCE_CANDIDATES);
    shortlist
}

fn push_unique<'a>(shortlist: &mut Vec<&'a search::SearchHit>, candidate: &'a search::SearchHit) {
    if !shortlist
        .iter()
        .any(|existing| existing.url == candidate.url)
    {
        shortlist.push(candidate);
    }
}
