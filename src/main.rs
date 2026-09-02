mod chain;
mod cli;
mod evidence;
mod face;
mod pipeline;
mod search;

use anyhow::Result;
use clap::Parser;
use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    pipeline::run(cli)
}
