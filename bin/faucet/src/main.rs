mod api;
mod api_key;
mod error_report;
mod logging;
mod network;
#[cfg(test)]
mod testing;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use miden_client::account::component::{AuthRpoFalcon512, BasicFungibleFaucet};
use miden_client::account::{
    Account,
    AccountBuilder,
    AccountFile,
    AccountStorageMode,
    AccountType,
};
use miden_client::asset::TokenSymbol;
use miden_client::auth::AuthSecretKey;
use miden_client::crypto::RpoRandomCoin;
use miden_client::crypto::rpo_falcon512::SecretKey;
use miden_client::rpc::Endpoint;
use miden_client::{Felt, Word};
use miden_client_sqlite_store::SqliteStore;
use miden_faucet_lib::types::AssetAmount;
use miden_faucet_lib::{Faucet, FaucetConfig};
use miden_pow_rate_limiter::PoWRateLimiterConfig;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use tokio::sync::mpsc;
use url::Url;

use crate::api::Server;
use crate::api_key::ApiKey;
use crate::logging::OpenTelemetry;
use crate::network::FaucetNetwork;

// CONSTANTS
// =================================================================================================

pub const REQUESTS_QUEUE_SIZE: usize = 1000;
const COMPONENT: &str = "miden-faucet-server";

const ENV_ENDPOINT: &str = "MIDEN_FAUCET_ENDPOINT";
const ENV_NETWORK: &str = "MIDEN_FAUCET_NETWORK";
const ENV_NODE_URL: &str = "MIDEN_FAUCET_NODE_URL";
const ENV_TIMEOUT: &str = "MIDEN_FAUCET_TIMEOUT";
const ENV_MAX_CLAIMABLE_AMOUNT: &str = "MIDEN_FAUCET_MAX_CLAIMABLE_AMOUNT";
const ENV_REMOTE_TX_PROVER_URL: &str = "MIDEN_FAUCET_REMOTE_TX_PROVER_URL";
const ENV_POW_SECRET: &str = "MIDEN_FAUCET_POW_SECRET";
const ENV_POW_CHALLENGE_LIFETIME: &str = "MIDEN_FAUCET_POW_CHALLENGE_LIFETIME";
const ENV_POW_CLEANUP_INTERVAL: &str = "MIDEN_FAUCET_POW_CLEANUP_INTERVAL";
const ENV_POW_GROWTH_RATE: &str = "MIDEN_FAUCET_POW_GROWTH_RATE";
const ENV_POW_BASELINE: &str = "MIDEN_FAUCET_POW_BASELINE";
const ENV_BASE_AMOUNT: &str = "MIDEN_FAUCET_BASE_AMOUNT";
const ENV_API_KEYS: &str = "MIDEN_FAUCET_API_KEYS";
const ENV_ENABLE_OTEL: &str = "MIDEN_FAUCET_ENABLE_OTEL";
const ENV_STORE: &str = "MIDEN_FAUCET_STORE";
const ENV_EXPLORER_URL: &str = "MIDEN_FAUCET_EXPLORER_URL";
const ENV_BATCH_SIZE: &str = "MIDEN_FAUCET_BATCH_SIZE";

// COMMANDS
// ================================================================================================

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Initialize the faucet with a new or existing account.
    Init {
        #[clap(flatten)]
        config: ClientConfig,

        /// Symbol of the new token.
        #[arg(
            short,
            long,
            value_name = "STRING",
            required_unless_present = "import_account_path"
        )]
        token_symbol: Option<String>,

        /// Decimals of the new token.
        #[arg(short, long, value_name = "U8", required_unless_present = "import_account_path")]
        decimals: Option<u8>,

        /// Max supply of the new token (in base units).
        #[arg(short, long, value_name = "U64", required_unless_present = "import_account_path")]
        max_supply: Option<u64>,

        /// Set an existing faucet account file to use, instead of creating a new account.
        #[arg(long = "import", value_name = "FILE", conflicts_with_all = ["token_symbol", "decimals", "max_supply"])]
        import_account_path: Option<PathBuf>,

        /// Whether to deploy the faucet account to the node.
        #[arg(short, long, value_name = "BOOL", default_value_t = false)]
        deploy: bool,
    },

    /// Generate an API key that can be used by the faucet.
    ///
    /// Prints out the generated API key to stdout. Keys can then be supplied to the faucet via the
    /// `--api-keys` flag or `MIDEN_FAUCET_API_KEYS` env var of the `start` command.
    CreateApiKey,

    /// Start the faucet server
    Start {
        #[clap(flatten)]
        config: ClientConfig,

        /// Endpoint of the faucet in the format `<ip>:<port>`.
        #[arg(long = "endpoint", value_name = "URL", env = ENV_ENDPOINT)]
        endpoint: Url,

        /// The maximum amount of assets' base units that can be dispersed on each request.
        #[arg(long = "max-claimable-amount", value_name = "U64", env = ENV_MAX_CLAIMABLE_AMOUNT, default_value = "1000000000")]
        max_claimable_amount: u64,

        /// The secret to be used by the server to sign the `PoW` challenges. This should NOT be
        /// shared.
        #[arg(long = "pow-secret", value_name = "STRING", default_value = "", env = ENV_POW_SECRET)]
        pow_secret: String,

        /// The duration during which the `PoW` challenges are valid. Changing this will affect the
        /// rate limiting, since it works by rejecting new submissions while the previous submitted
        /// challenge is still valid.
        #[arg(long = "pow-challenge-lifetime", value_name = "DURATION", env = ENV_POW_CHALLENGE_LIFETIME, default_value = "30s", value_parser = humantime::parse_duration)]
        pow_challenge_lifetime: Duration,

        /// Defines how quickly the `PoW` difficulty grows with the number of requests. The number
        /// of active challenges gets multiplied by the growth rate to compute the load
        /// difficulty.
        ///
        /// Meaning, the difficulty bits of the challenge will increase approximately by
        /// `log2(growth_rate * num_active_challenges)`.
        #[arg(long = "pow-growth-rate", value_name = "F64", env = ENV_POW_GROWTH_RATE, default_value = "0.1")]
        pow_growth_rate: f64,

        /// The interval at which the `PoW` challenge cache is cleaned up.
        #[arg(long = "pow-cleanup-interval", value_name = "DURATION", env = ENV_POW_CLEANUP_INTERVAL, default_value = "2s", value_parser = humantime::parse_duration)]
        pow_cleanup_interval: Duration,

        /// The baseline for the `PoW` challenges. This sets the `PoW` difficulty (in bits) that a
        /// a challenge will have when there are no requests against the faucet. It must be between
        /// 0 and 32.
        #[arg(value_parser = clap::value_parser!(u8).range(0..=32))]
        #[arg(long = "pow-baseline", value_name = "U8", env = ENV_POW_BASELINE, default_value = "16")]
        pow_baseline: u8,

        /// The baseline amount for token requests (in base units). Requests for greater amounts
        /// would require higher level of `PoW`.
        ///
        /// The request complexity for challenges is computed as: `request_complexity = (amount /
        /// base_amount) + 1`
        #[arg(long = "base-amount", value_name = "U64", env = ENV_BASE_AMOUNT, default_value = "100000000")]
        base_amount: u64,

        /// Comma-separated list of API keys.
        #[arg(long = "api-keys", value_name = "STRING", env = ENV_API_KEYS, num_args = 1.., value_delimiter = ',')]
        api_keys: Vec<String>,

        /// Enables the exporting of traces for OpenTelemetry.
        ///
        /// This can be further configured using environment variables as defined in the official
        /// OpenTelemetry documentation. See our operator manual for further details.
        #[arg(long = "enable-otel", value_name = "BOOL", default_value_t = false, env = ENV_ENABLE_OTEL)]
        open_telemetry: bool,

        /// Explorer URL.
        #[arg(long = "explorer-url", value_name = "URL", env = ENV_EXPLORER_URL)]
        explorer_url: Option<Url>,

        /// The maximum number of requests to process in each batch. Each batch is processed in a
        /// single transaction.
        #[arg(long = "batch-size", value_name = "USIZE", default_value = "32", env = ENV_BATCH_SIZE)]
        batch_size: usize,
    },
}

/// Configuration for the faucet client.
#[derive(Parser, Debug, Clone)]
pub struct ClientConfig {
    /// Path to the `SQLite` store.
    #[arg(long = "store", value_name = "FILE", default_value = "faucet_client_store.sqlite3", env = ENV_STORE)]
    store_path: PathBuf,

    /// Timeout for attempting to connect to the node.
    #[arg(long = "timeout", value_name = "DURATION", default_value = "5s", env = ENV_TIMEOUT, value_parser = humantime::parse_duration)]
    timeout: Duration,

    /// Network configuration to use. Options are `devnet`, `testnet`, `localhost` or a custom
    /// network. It is used to display the correct bech32 addresses in the UI.
    #[arg(long = "network", value_name = "NETWORK", default_value = "localhost", env = ENV_NETWORK)]
    network: FaucetNetwork,

    /// Endpoint of the remote transaction prover in the format `<protocol>://<host>[:<port>]`.
    #[arg(long = "remote-tx-prover-url", value_name = "URL", env = ENV_REMOTE_TX_PROVER_URL)]
    remote_tx_prover_url: Option<Url>,

    /// Node RPC gRPC endpoint in the format `http://<host>[:<port>]`. If not set, the url is derived
    /// from the specified network.
    #[arg(long = "node-url", value_name = "URL", env = ENV_NODE_URL)]
    node_url: Option<Url>,
}

impl Command {
    fn open_telemetry(&self) -> OpenTelemetry {
        if match *self {
            Command::Start { open_telemetry, .. } => open_telemetry,
            _ => false,
        } {
            OpenTelemetry::Enabled
        } else {
            OpenTelemetry::Disabled
        }
    }
}

// MAIN
// =================================================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Configure tracing with optional OpenTelemetry exporting support.
    let _otel_guard = logging::setup_tracing(cli.command.open_telemetry())
        .context("failed to initialize logging")?;

    Box::pin(run_faucet_command(cli)).await
}

#[allow(clippy::too_many_lines)]
async fn run_faucet_command(cli: Cli) -> anyhow::Result<()> {
    // Note: open-telemetry is handled in main.
    match cli.command {
        Command::Init {
            config:
                ClientConfig {
                    node_url,
                    timeout,
                    remote_tx_prover_url,
                    network,
                    store_path,
                },
            token_symbol,
            decimals,
            max_supply,
            import_account_path,
            deploy,
        } => {
            let (account, secret) = if let Some(account_path) = import_account_path {
                // Import existing faucet account
                let account_data = AccountFile::read(account_path)
                    .context("failed to read account data from file")?;
                let secret = account_data
                    .auth_secret_keys
                    .first()
                    .context("auth secret key is required")?
                    .clone();
                (account_data.account, secret)
            } else {
                println!("Generating new faucet account. This may take a few seconds...");
                let token_symbol =
                    token_symbol.expect("token_symbol should be present when not importing");
                let decimals = decimals.expect("decimals should be present when not importing");
                let max_supply =
                    max_supply.expect("max_supply should be present when not importing");
                create_faucet_account(token_symbol.as_str(), max_supply, decimals)?
            };
            let node_endpoint = parse_node_endpoint(node_url, &network)?;
            let faucet_config = FaucetConfig {
                store_path,
                node_endpoint,
                network_id: network.to_network_id()?,
                timeout,
                remote_tx_prover_url,
            };
            Box::pin(Faucet::init(&faucet_config, account, &secret, deploy))
                .await
                .context("failed to initialize faucet")?;

            println!("Faucet account successfully initialized");
        },

        Command::CreateApiKey => {
            let mut rng = ChaCha20Rng::from_seed(rand::random());
            let key = ApiKey::generate(&mut rng).encode();
            println!("{key}");
        },

        Command::Start {
            config:
                ClientConfig {
                    node_url,
                    timeout,
                    remote_tx_prover_url,
                    network,
                    store_path,
                },
            endpoint,
            max_claimable_amount,
            pow_secret,
            pow_challenge_lifetime,
            pow_cleanup_interval,
            pow_growth_rate,
            pow_baseline,
            base_amount,
            api_keys,
            open_telemetry: _,
            explorer_url,
            batch_size,
        } => {
            let node_endpoint = parse_node_endpoint(node_url, &network)?;
            let config = FaucetConfig {
                store_path: store_path.clone(),
                node_endpoint,
                network_id: network.to_network_id()?,
                timeout,
                remote_tx_prover_url,
            };
            let faucet = Faucet::load(&config).await.context("failed to load faucet")?;

            let store =
                Arc::new(SqliteStore::new(store_path).await.context("failed to create store")?);

            // Maximum of 1000 requests in-queue at once. Overflow is rejected for faster feedback.
            let (tx_mint_requests, rx_mint_requests) = mpsc::channel(REQUESTS_QUEUE_SIZE);

            let api_keys = api_keys
                .iter()
                .map(|k| ApiKey::decode(k))
                .collect::<Result<Vec<_>, _>>()
                .context("failed to decode API keys")?;
            let max_claimable_amount = AssetAmount::new(max_claimable_amount)?;
            let rate_limiter_config = PoWRateLimiterConfig {
                challenge_lifetime: pow_challenge_lifetime,
                cleanup_interval: pow_cleanup_interval,
                growth_rate: pow_growth_rate,
                baseline: pow_baseline,
            };

            let faucet_account = faucet.faucet_account().await?;
            let faucet_component = BasicFungibleFaucet::try_from(&faucet_account)?;
            let max_supply = AssetAmount::new(faucet_component.max_supply().as_int())?;
            let decimals = faucet_component.decimals();

            let server = Server::new(
                faucet.faucet_id(),
                decimals,
                max_supply,
                faucet.issuance(),
                max_claimable_amount,
                base_amount,
                tx_mint_requests,
                pow_secret.as_str(),
                rate_limiter_config,
                &api_keys,
                store,
                explorer_url,
            );

            // Use select to concurrently:
            // - Run and wait for the faucet (on current thread)
            // - Run and wait for server (in a spawned task)
            let faucet_future = faucet.run(rx_mint_requests, batch_size);
            let server_future = async {
                let server_handle =
                    tokio::spawn(
                        async move { server.serve(endpoint).await.context("server failed") },
                    );
                server_handle.await.context("failed to join server task")?
            };

            tokio::select! {
                server_result = server_future => {
                    // If server completes first, return its result
                    server_result.context("server failed")
                },
                faucet_result = faucet_future => {
                    // Faucet completed, return its result
                    faucet_result.context("faucet failed")
                },
            }?;
        },
    }

    Ok(())
}

// UTILITIES
// =================================================================================================

/// Parses the node endpoint from the cli arguments. If an explicit url is provided, it is used.
/// Otherwise, it is derived from the specified network.
fn parse_node_endpoint(node_url: Option<Url>, network: &FaucetNetwork) -> anyhow::Result<Endpoint> {
    let url = if let Some(node_url) = node_url {
        node_url.to_string()
    } else {
        network
            .to_rpc_endpoint()
            .context("no node url provided for the custom network")?
    };

    Endpoint::try_from(url.as_str())
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("failed to parse node url: {url}"))
}

/// Creates a new faucet account from the given parameters.
fn create_faucet_account(
    token_symbol: &str,
    max_supply: u64,
    decimals: u8,
) -> anyhow::Result<(Account, AuthSecretKey)> {
    let mut rng = ChaCha20Rng::from_seed(rand::random());
    let secret = {
        let auth_seed: [u64; 4] = rng.random();
        let rng_seed = Word::from(auth_seed.map(Felt::new));
        SecretKey::with_rng(&mut RpoRandomCoin::new(rng_seed))
    };

    let symbol = TokenSymbol::try_from(token_symbol).context("failed to parse token symbol")?;
    let max_supply = Felt::try_from(max_supply)
        .map_err(anyhow::Error::msg)
        .context("max supply value is greater than or equal to the field modulus")?;

    let account = AccountBuilder::new(rng.random())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply)?)
        .with_auth_component(AuthRpoFalcon512::new(secret.public_key()))
        .build()
        .context("failed to create basic fungible faucet account")?;

    Ok((account, AuthSecretKey::RpoFalcon512(secret)))
}

#[cfg(test)]
mod test {
    use std::env::temp_dir;
    use std::process::Stdio;
    use std::str::FromStr;
    use std::time::{Duration, Instant};

    use fantoccini::ClientBuilder;
    use miden_client::account::{
        AccountId,
        AccountIdAddress,
        Address,
        AddressInterface,
        NetworkId,
    };
    use serde_json::{Map, json};
    use tokio::io::AsyncBufReadExt;
    use tokio::time::sleep;
    use url::Url;

    use crate::network::FaucetNetwork;
    use crate::testing::stub_rpc_api::serve_stub;
    use crate::{Cli, ClientConfig, run_faucet_command};

    /// This test starts a stub node, a faucet connected to the stub node, and a chromedriver
    /// to test the faucet website. It then loads the website and checks that all the requests
    /// made return status 200.
    #[tokio::test]
    async fn test_website() {
        let website_url = Box::pin(start_test_faucet()).await;
        let client = start_fantoccini_client().await;

        // Open the website
        client.goto(website_url.as_str()).await.unwrap();

        let title = client.title().await.unwrap();
        assert_eq!(title, "Miden Faucet");

        let network_id = NetworkId::Testnet;
        let account_id = AccountId::try_from(0).unwrap();
        let address =
            Address::from(AccountIdAddress::new(account_id, AddressInterface::BasicWallet));
        let address_bech32 = address.to_bech32(network_id);

        // Fill in the account address
        client
            .find(fantoccini::Locator::Css("#recipient-address"))
            .await
            .unwrap()
            .send_keys(&address_bech32)
            .await
            .unwrap();

        // Select the first asset amount option
        client
            .find(fantoccini::Locator::Css("#token-amount"))
            .await
            .unwrap()
            .click()
            .await
            .unwrap();
        client
            .find(fantoccini::Locator::Css("#token-amount option"))
            .await
            .unwrap()
            .click()
            .await
            .unwrap();

        // Click the public note button
        client
            .find(fantoccini::Locator::Css("#send-public-button"))
            .await
            .unwrap()
            .click()
            .await
            .unwrap();

        // Execute a script to get all the failed requests
        let script = r"
            let errors = [];
            performance.getEntriesByType('resource').forEach(entry => {
                if (entry.responseStatus && entry.responseStatus >= 400) {
                    errors.push({url: entry.name, status: entry.responseStatus});
                }
            });
            return errors;
        ";
        let failed_requests = client.execute(script, vec![]).await.unwrap();

        // Verify all requests are successful
        assert!(failed_requests.as_array().unwrap().is_empty());

        client.close().await.unwrap();
    }

    async fn start_test_faucet() -> Url {
        let stub_node_url = Url::from_str("http://localhost:50051").unwrap();
        let config = ClientConfig {
            node_url: Some(stub_node_url.clone()),
            timeout: Duration::from_millis(5000),
            network: FaucetNetwork::Localhost,
            store_path: temp_dir().join("test_store.sqlite3"),
            remote_tx_prover_url: None,
        };

        // Start the stub node
        tokio::spawn(async move {
            serve_stub(&stub_node_url).await.expect("failed to start stub node");
        });

        // Create faucet account and initialize the faucet
        Box::pin(run_faucet_command(Cli {
            command: crate::Command::Init {
                config: config.clone(),
                token_symbol: Some("TEST".to_owned()),
                decimals: Some(6),
                max_supply: Some(1_000_000_000_000),
                import_account_path: None,
                deploy: false,
            },
        }))
        .await
        .expect("failed to create faucet account");

        // Start the faucet connected to the stub
        // Use std::thread to launch faucet - avoids Send requirements
        let endpoint_clone = Url::parse("http://localhost:8080").unwrap();
        std::thread::spawn(move || {
            // Create a new runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build runtime");

            // Run the faucet on this thread's runtime
            rt.block_on(async {
                Box::pin(run_faucet_command(Cli {
                    command: crate::Command::Start {
                        config,
                        endpoint: endpoint_clone,
                        max_claimable_amount: 1_000_000_000,
                        api_keys: vec![],
                        pow_secret: "test".to_string(),
                        pow_challenge_lifetime: Duration::from_secs(30),
                        pow_cleanup_interval: Duration::from_secs(1),
                        pow_growth_rate: 1.0,
                        pow_baseline: 12,
                        base_amount: 100_000,
                        open_telemetry: false,
                        explorer_url: None,
                        batch_size: 8,
                    },
                }))
                .await
                .expect("failed to start faucet");
            });
        });

        // Wait for faucet to be up
        let endpoint = Url::parse("http://localhost:8080").unwrap();
        let addrs = endpoint.socket_addrs(|| None).unwrap();
        let start = Instant::now();
        let timeout = Duration::from_secs(10);
        loop {
            match tokio::net::TcpStream::connect(&addrs[..]).await {
                Ok(_) => break,
                Err(_) if start.elapsed() < timeout => {
                    sleep(Duration::from_millis(200)).await;
                },
                Err(e) => panic!("faucet never became reachable: {e}"),
            }
        }

        endpoint
    }

    async fn start_fantoccini_client() -> fantoccini::Client {
        // Start chromedriver. This requires having chromedriver and chrome installed
        let chromedriver_port = "57708";
        let mut chromedriver = tokio::process::Command::new("chromedriver")
            .arg(format!("--port={chromedriver_port}"))
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("failed to start chromedriver");
        let stdout = chromedriver.stdout.take().unwrap();
        tokio::spawn(
            async move { chromedriver.wait().await.expect("chromedriver process failed") },
        );
        // Wait for chromedriver to be running
        let mut reader = tokio::io::BufReader::new(stdout).lines();
        while let Some(line) = reader.next_line().await.unwrap() {
            if line.contains("ChromeDriver was started successfully") {
                break;
            }
        }

        // Start fantoccini client
        ClientBuilder::native()
            .capabilities(
                [(
                    "goog:chromeOptions".to_string(),
                    json!({"args": ["--headless", "--disable-gpu", "--no-sandbox"]}),
                )]
                .into_iter()
                .collect::<Map<_, _>>(),
            )
            .connect(&format!("http://localhost:{chromedriver_port}"))
            .await
            .expect("failed to connect to WebDriver")
    }
}
