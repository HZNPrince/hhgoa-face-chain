use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "hhgoa-face-chain")]
#[command(about = "Face scan -> web/social discovery -> blockchain verification pipeline")]
pub struct Cli {
    #[arg(long, value_name = "PATH")]
    pub image: PathBuf,

    #[arg(long, value_enum, default_value_t = SearchProvider::Fixture)]
    pub search_provider: SearchProvider,

    #[arg(long, env = "SERPAPI_KEY")]
    pub serpapi_key: Option<String>,

    #[arg(long, value_name = "URL")]
    pub image_url: Option<String>,

    #[arg(
        long,
        value_name = "PATH",
        default_value = "fixtures/search_result.json"
    )]
    pub fixture: PathBuf,

    #[arg(long, value_enum, default_value_t = ChainProvider::Local)]
    pub chain_provider: ChainProvider,

    #[arg(long, value_name = "PATH", default_value = "data/local_chain.json")]
    pub local_chain: PathBuf,

    #[arg(long, default_value = "devnet")]
    pub solana_cluster: String,

    #[arg(long)]
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
