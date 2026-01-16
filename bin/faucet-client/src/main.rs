use clap::{Parser, Subcommand};
use miden_faucet_client::mint;

/// Client CLI for interacting with a live faucet.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Request tokens from a remote faucet (does not consume the resulting note).
    Mint(mint::MintCmd),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Mint(cmd) => {
            cmd.execute().await.map_err(anyhow::Error::from)?;
        },
    }

    Ok(())
}
