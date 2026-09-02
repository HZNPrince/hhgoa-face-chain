# HH Goa Face Chain

Rust pipeline for **HH Goa 2026 Shortlisting Task 3: Face Identification & Blockchain Verification**.

The pipeline runs:

```text
face scan image -> reverse image search candidates -> face verification -> canonical evidence hash -> blockchain record -> verification
```

## Why this design

Social platforms usually do not expose public APIs that let you search Instagram, X, or Facebook by a raw face vector. The practical route is reverse image search: send the image or face crop to a visual search provider, receive public pages or posts where visually matching media appears, then store a tamper-evident hash of the discovered result on-chain.

This repo keeps the pipeline modular:

- `face`: detects a likely face region and creates a deterministic perceptual encoding.
- `search`: supports an offline fixture for development and SerpAPI Google Lens for a real reverse-image search.
- `search` also verifies candidate images before accepting a result, so visually similar but wrong people are rejected before anything is written on-chain.
- `evidence`: canonicalizes the found post metadata and hashes it with SHA-256.
- `chain`: supports a local simulated blockchain and Solana Memo transactions.

## Quick start

Add a clear portrait image at `samples/input.jpg`, then run:

```bash
cargo run -- \
  --image samples/input.jpg \
  --search-provider fixture \
  --chain-provider local
```

The command writes:

- `data/evidence.json`
- `data/report.json`
- `data/local_chain.json`

## Real reverse image search

For the final recording, use SerpAPI Google Lens:

```bash
export SERPAPI_KEY=your_key_here

cargo run -- \
  --image samples/input.jpg \
  --image-url "https://public-url-to-the-same-image.jpg" \
  --search-provider serpapi \
  --chain-provider local \
  --min-face-similarity 0.64
```

SerpAPI Google Lens needs a public image URL. If the scan is only local, upload it to a temporary public location first.

For the final demo, use a public image/post that is strongly indexed. The current reliable demo pattern is a known public figure image from an Instagram/X/news post where the public `og:image` contains a clear face. The project intentionally rejects candidates that do not pass face verification.

## Solana verification

The Solana path stores the evidence hash in a Solana Memo transaction:

```bash
solana config set --url devnet
solana airdrop 1

cargo run -- \
  --image samples/input.jpg \
  --image-url "https://public-url-to-the-same-image.jpg" \
  --search-provider serpapi \
  --chain-provider solana-memo \
  --solana-cluster devnet \
  --min-face-similarity 0.64
```

Verification re-fetches the transaction with `solana confirm -v` and checks that the memo contains the expected evidence hash.

## Screen recording script

1. Show the input image.
2. Run the SerpAPI command so the terminal shows face scan, candidate scores, selected verified result, evidence hash, and chain record.
3. Open `data/report.json` to show the structured result.
4. If using Solana, run `solana confirm -v <signature>` and show the memo contains the same hash.

## Known limitations

- The default local face scanner is a lightweight Rust heuristic designed for clear portrait images. For production, replace it with a stronger model or API such as InsightFace, AWS Rekognition, Azure Face, or a Rust ONNX model.
- Fixture search is for local development only. The final submission should use `--search-provider serpapi` or another genuine reverse-image search provider.
- Public testnet Solana submission requires a funded devnet wallet and network access.
- Use consented images or public figures/public posts for the demo.
