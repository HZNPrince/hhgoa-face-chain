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
- Marks only successful candidate face checks as `VERIFIED`; other results remain normal discovered sources.
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
solana config set --url devnet

cargo run -- --image samples/input.jpg
```

The default settings are:

- `FACE_CHAIN_SEARCH_PROVIDER=serpapi`
- `FACE_CHAIN_CHAIN_PROVIDER=solana-memo`
- `FACE_CHAIN_SOLANA_CLUSTER=devnet`
- `FACE_CHAIN_MIN_FACE_SIMILARITY=0.64`

You can override them with flags or environment variables. If the image is already public, pass its URL like this:

```bash
cargo run -- \
  --image samples/input.jpg \
  --image-url "https://example.com/image.jpg"
```

Outputs:

- `data/evidence.json`
- `data/report.json`

## Blockchain Used

The submission demo uses **Solana devnet memo transactions**:

- The pipeline creates a SHA-256 hash of `data/evidence.json`.
- It submits a tiny Solana devnet transfer to the configured wallet.
- The evidence hash is stored in the transaction memo as `hhgoa-face-chain:<hash>`.
- Verification fetches the transaction with `solana confirm -v` and checks that the memo contains the expected hash.

```bash
solana config set --url devnet
solana airdrop 1

cargo run -- \
  --image samples/input.jpg
```

The project also includes a local simulated chain for offline development:

```bash
cargo run -- \
  --image samples/input.jpg \
  --search-provider fixture \
  --chain-provider local
```

Local mode appends blocks to `data/local_chain.json`, but the final recording should use `--chain-provider solana-memo`.

## Known Limitations

- The built-in face scanner is a lightweight Rust heuristic, not a production-grade face recognition model.
- Reverse image search depends on what Google Lens/SerpAPI can index publicly.
- Social media platforms may block full media downloads, so some discovered social matches may not receive a `VERIFIED` label.
- SerpAPI local image upload supports JPG, PNG, and WebP files up to 500 KB; larger inputs are compressed before upload.
- Solana devnet requires the Solana CLI, a configured devnet keypair, and enough devnet SOL for a small memo transaction.
- Use public figures, public posts, or consented images for demos.
