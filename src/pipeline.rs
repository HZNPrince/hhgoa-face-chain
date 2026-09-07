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
const LINKS_PATH: &str = "data/links.html";
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
    let explorer_url = solana_explorer_url(&receipt);
    if let Some(explorer_url) = &explorer_url {
        println!(
            "    Explorer       {}",
            clickable_link(explorer_url, explorer_url)
        );
    }
    write_links_report(
        selected,
        &search_report.candidates,
        &evidence_hash,
        &receipt,
        explorer_url.as_deref(),
    )?;
    println!("    Links report   {LINKS_PATH}");

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
    println!("  Links file      {LINKS_PATH}");
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

fn write_links_report(
    selected: &search::SearchHit,
    candidates: &[search::SearchHit],
    evidence_hash: &str,
    receipt: &chain::ChainReceipt,
    explorer_url: Option<&str>,
) -> Result<()> {
    let mut html = String::new();
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    html.push_str("<title>HH Goa Face Chain Links</title>");
    html.push_str(
        "<style>
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;margin:32px;line-height:1.45;color:#111827;background:#f8fafc}
main{max-width:980px;margin:0 auto}
h1{margin-bottom:4px}
.muted{color:#64748b}
.card{background:white;border:1px solid #e5e7eb;border-radius:8px;padding:16px;margin:14px 0}
.meta{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:13px;color:#334155;word-break:break-all}
a{color:#0f63c7;text-decoration:none}
a:hover{text-decoration:underline}
.status{display:inline-block;font-size:12px;font-weight:700;color:#166534;background:#dcfce7;border-radius:999px;padding:3px 8px}
.discovered{color:#475569;background:#f1f5f9}
ol{padding-left:22px}
li{margin:12px 0}
</style>",
    );
    html.push_str("</head><body><main>");
    html.push_str("<h1>HH Goa Face Chain</h1>");
    html.push_str("<p class=\"muted\">Clickable proof links generated by the pipeline.</p>");

    html.push_str("<section class=\"card\"><h2>Selected Match</h2>");
    html.push_str(&format!(
        "<p><span class=\"status {}\">{}</span></p>",
        if selected.face_verified {
            ""
        } else {
            "discovered"
        },
        html_escape(format_check(selected))
    ));
    html.push_str(&format!(
        "<p><strong>{}</strong></p><p><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a></p>",
        html_escape(&selected.title),
        html_escape(&selected.url),
        html_escape(&selected.url)
    ));
    html.push_str(&format!(
        "<p class=\"muted\">Source: {}</p>",
        html_escape(&selected.source)
    ));
    html.push_str("</section>");

    html.push_str("<section class=\"card\"><h2>Blockchain Proof</h2>");
    html.push_str(&format!(
        "<p class=\"meta\">Evidence SHA-256: {}</p><p class=\"meta\">Record: {}</p><p class=\"meta\">Verification: {}</p>",
        html_escape(evidence_hash),
        html_escape(&receipt.record_id),
        html_escape(&receipt.verification)
    ));
    if let Some(explorer_url) = explorer_url {
        html.push_str(&format!(
            "<p><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">Open Solana Explorer transaction</a></p>",
            html_escape(explorer_url)
        ));
    }
    html.push_str("</section>");

    html.push_str("<section class=\"card\"><h2>Discovered Candidates</h2><ol>");
    for candidate in candidates.iter().take(MAX_EVIDENCE_CANDIDATES) {
        html.push_str(&format!(
            "<li><strong>{}</strong><br><span class=\"muted\">{}</span><br><a href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a></li>",
            html_escape(&candidate.title),
            html_escape(&candidate.source),
            html_escape(&candidate.url),
            html_escape(&candidate.url)
        ));
    }
    html.push_str("</ol></section>");
    html.push_str("</main></body></html>");

    std::fs::write(LINKS_PATH, html).context("writing data/links.html")?;
    Ok(())
}

fn html_escape(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
