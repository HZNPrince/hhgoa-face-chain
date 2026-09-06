use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "hhgoa-face-chain")]
#[command(about = "Face scan -> web/social discovery -> blockchain verification pipeline")]
pub struct Cli {
    #[arg(long, value_name = "PATH")]
    pub image: PathBuf,

    #[arg(
        long,
        env = "FACE_CHAIN_SEARCH_PROVIDER",
        value_enum,
        default_value_t = SearchProvider::Serpapi
    )]
    pub search_provider: SearchProvider,

    #[arg(long, env = "SERPAPI_KEY")]
    pub serpapi_key: Option<String>,

    #[arg(long, env = "FACE_CHAIN_IMAGE_URL", value_name = "URL")]
    pub image_url: Option<String>,

    #[arg(long, env = "FACE_CHAIN_MIN_FACE_SIMILARITY", default_value_t = 0.64)]
    pub min_face_similarity: f32,

    #[arg(
        long,
        value_name = "PATH",
        default_value = "fixtures/search_result.json"
    )]
    pub fixture: PathBuf,

    #[arg(
        long,
        env = "FACE_CHAIN_CHAIN_PROVIDER",
        value_enum,
        default_value_t = ChainProvider::SolanaMemo
    )]
    pub chain_provider: ChainProvider,

    #[arg(
        long,
        env = "FACE_CHAIN_LOCAL_CHAIN",
        value_name = "PATH",
        default_value = "data/local_chain.json"
    )]
    pub local_chain: PathBuf,

    #[arg(long, env = "FACE_CHAIN_SOLANA_CLUSTER", default_value = "devnet")]
    pub solana_cluster: String,

    #[arg(long, env = "FACE_CHAIN_SKIP_VERIFY")]
    pub skip_verify: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum SearchProvider {
    Fixture,
    Serpapi,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ChainProvider {
    Local,
    SolanaMemo,
}
