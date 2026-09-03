# HH Goa Face Chain

Rust implementation for **HH Goa 2026 Shortlisting Task 3: Face Identification & Blockchain Verification**.

## Functionality

This project runs an end-to-end pipeline:

```text
face image -> reverse image search -> web/social candidates -> evidence hash -> blockchain record -> verification
```

What it does:

- Detects a face-like region from an input image and creates a deterministic face fingerprint.
- Uses SerpAPI Google Lens to run a real reverse image search from either a local uploaded image or a public image URL.
- Prints top web results and social candidates separately.
- Marks candidate face checks as `PASS`, `MISMATCH`, or `UNVERIFIED`.
- Builds `data/evidence.json` with the selected match plus multiple discovered candidates.
- Hashes the evidence JSON with SHA-256.
- Stores and re-verifies that hash on a blockchain layer.

## How To Run

Install Rust, then clone and run:

```bash
git clone git@github.com:HZNPrince/hhgoa-face-chain.git
cd hhgoa-face-chain
cargo test
```

Offline fixture demo:

```bash
cargo run -- \
  --image samples/input.jpg \
  --search-provider fixture \
  --chain-provider local
```

Real reverse-image demo:

```bash
export SERPAPI_KEY=your_serpapi_key

cargo run -- \
  --image samples/input.jpg \
  --search-provider serpapi \
  --chain-provider local \
  --min-face-similarity 0.64
```

If the image is already public, you can pass its URL:

```bash
cargo run -- \
  --image samples/input.jpg \
  --image-url "https://example.com/image.jpg" \
  --search-provider serpapi \
  --chain-provider local \
  --min-face-similarity 0.64
```

Outputs:

- `data/evidence.json`
- `data/report.json`
- `data/local_chain.json`

## Blockchain Used

The default demo uses a **local simulated blockchain**:

- Each run appends a block to `data/local_chain.json`.
- Each block stores the SHA-256 hash of the evidence.
- Verification recomputes the block hash and confirms the evidence hash exists in the chain.

The project also includes an optional **Solana devnet memo** mode:

```bash
solana config set --url devnet
solana airdrop 1

cargo run -- \
  --image samples/input.jpg \
  --search-provider serpapi \
  --chain-provider solana-memo \
  --solana-cluster devnet \
  --min-face-similarity 0.64
```

For the hackathon recording, local chain mode is recommended because it is deterministic and does not depend on devnet availability.

## Known Limitations

- The built-in face scanner is a lightweight Rust heuristic, not a production-grade face recognition model.
- Reverse image search depends on what Google Lens/SerpAPI can index publicly.
- Social media platforms may block full media downloads, so some social matches are marked `UNVERIFIED`.
- SerpAPI local image upload supports JPG, PNG, and WebP files up to 500 KB.
- Use public figures, public posts, or consented images for demos.
