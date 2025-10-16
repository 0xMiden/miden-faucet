mod api_key;
mod backend;
mod frontend;
mod logging;
mod network;
#[cfg(test)]
mod testing;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use miden_client::account::component::{AuthRpoFalcon512, BasicFungibleFaucet};
use miden_client::account::{AccountBuilder, AccountFile, AccountStorageMode, AccountType};
use miden_client::asset::TokenSymbol;
use miden_client::auth::AuthSecretKey;
use miden_client::crypto::RpoRandomCoin;
use miden_client::crypto::rpo_falcon512::SecretKey;
use miden_client::{Felt, Word};
use miden_client_sqlite_store::SqliteStore;
use miden_faucet_lib::Faucet;
use miden_faucet_lib::types::AssetAmount;
use miden_pow_rate_limiter::PoWRateLimiterConfig;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use url::Url;

use crate::api_key::ApiKey;
use crate::backend::{BackendServer, Metadata};
use crate::frontend::serve_frontend;
use crate::logging::OpenTelemetry;
use crate::network::FaucetNetwork;

// CONSTANTS
// =================================================================================================

pub const REQUESTS_QUEUE_SIZE: usize = 1000;
const COMPONENT: &str = "miden-faucet-server";

const ENV_BACKEND_URL: &str = "MIDEN_FAUCET_BACKEND_URL";
const ENV_FRONTEND_URL: &str = "MIDEN_FAUCET_FRONTEND_URL";
const ENV_NODE_URL: &str = "MIDEN_FAUCET_NODE_URL";
const ENV_TIMEOUT: &str = "MIDEN_FAUCET_TIMEOUT";
const ENV_ACCOUNT_PATH: &str = "MIDEN_FAUCET_ACCOUNT_PATH";
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
const ENV_NETWORK: &str = "MIDEN_FAUCET_NETWORK";
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
    /// Start the faucet server
    Start {
        /// Backend API URL, in the format `<ip>:<port>`.
        #[arg(long = "backend-url", value_name = "URL", env = ENV_BACKEND_URL)]
        backend_url: Url,

        /// Frontend API URL, in the format `<ip>:<port>`. If not set, the frontend will not be
        /// served.
        #[arg(long = "frontend-url", value_name = "URL", env = ENV_FRONTEND_URL)]
        frontend_url: Option<Url>,

        /// Node RPC gRPC endpoint in the format `http://<host>[:<port>]`.
        #[arg(long = "node-url", value_name = "URL", env = ENV_NODE_URL)]
        node_url: Url,

        /// Timeout for RPC requests.
        #[arg(long = "timeout", value_name = "DURATION", default_value = "5s", env = ENV_TIMEOUT, value_parser = humantime::parse_duration)]
        timeout: Duration,

        /// Path to the faucet account file.
        #[arg(long = "account", value_name = "FILE", env = ENV_ACCOUNT_PATH)]
        faucet_account_path: PathBuf,

        /// The maximum amount of assets' base units that can be dispersed on each request.
        #[arg(long = "max-claimable-amount", value_name = "U64", env = ENV_MAX_CLAIMABLE_AMOUNT, default_value = "1000000000")]
        max_claimable_amount: u64,

        /// Endpoint of the remote transaction prover in the format `<protocol>://<host>[:<port>]`.
        #[arg(long = "remote-tx-prover-url", value_name = "URL", env = ENV_REMOTE_TX_PROVER_URL)]
        remote_tx_prover_url: Option<Url>,

        /// Network configuration to use. Options are `devnet`, `testnet`, `localhost` or a custom
        /// network. It is used to display the correct bech32 addresses in the UI.
        #[arg(long = "network", value_name = "NETWORK", default_value = "localhost", env = ENV_NETWORK)]
        network: FaucetNetwork,

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

        /// Path to the `SQLite` store.
        #[arg(long = "store", value_name = "FILE", default_value = "faucet_client_store.sqlite3", env = ENV_STORE)]
        store_path: PathBuf,

        /// Explorer URL.
        #[arg(long = "explorer-url", value_name = "URL", env = ENV_EXPLORER_URL)]
        explorer_url: Option<Url>,

        /// The maximum number of requests to process in each batch. Each batch is processed in a
        /// single transaction.
        #[arg(long = "batch-size", value_name = "USIZE", default_value = "32", env = ENV_BATCH_SIZE)]
        batch_size: usize,
    },

    /// Create a new public faucet account and save to the specified file.
    CreateFaucetAccount {
        #[arg(short, long, value_name = "FILE")]
        output_path: PathBuf,
        #[arg(short, long, value_name = "STRING")]
        token_symbol: String,
        #[arg(short, long, value_name = "U8")]
        decimals: u8,
        #[arg(short, long, value_name = "U64")]
        max_supply: u64,
    },

    /// Generate API keys that can be used by the faucet.
    ///
    /// Prints out the specified number of API keys to stdout as a comma-separated list.
    /// This list can be supplied to the faucet via the `--api-keys` flag or `MIDEN_FAUCET_API_KEYS`
    /// env var of the start command.
    CreateApiKeys {
        #[arg()]
        count: u8,
    },
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
    match cli.command {
        // Note: open-telemetry is handled in main.
        Command::Start {
            backend_url,
            frontend_url,
            node_url,
            timeout,
            faucet_account_path,
            remote_tx_prover_url,
            network,
            max_claimable_amount,
            pow_secret,
            pow_challenge_lifetime,
            pow_cleanup_interval,
            pow_growth_rate,
            pow_baseline,
            base_amount,
            api_keys,
            open_telemetry: _,
            store_path,
            explorer_url,
            batch_size,
        } => {
            let account_file = AccountFile::read(&faucet_account_path).context(format!(
                "failed to load faucet account from file ({})",
                faucet_account_path.display()
            ))?;
            let faucet_component = BasicFungibleFaucet::try_from(&account_file.account)?;
            let max_supply = AssetAmount::new(faucet_component.max_supply().as_int())?;
            let decimals = faucet_component.decimals();

            let faucet = Faucet::load(
                store_path.clone(),
                network.to_network_id()?,
                account_file,
                &node_url,
                timeout,
                remote_tx_prover_url,
            )
            .await
            .context("failed to load faucet")?;

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
            let metadata = Metadata {
                id: faucet.faucet_id(),
                issuance: faucet.issuance(),
                max_supply,
                decimals,
                explorer_url,
                base_amount,
            };
            // We keep a channel sender open in the main thread to avoid the faucet closing before
            // servers can propagate any errors.
            let tx_mint_requests_clone = tx_mint_requests.clone();
            let backend_server = BackendServer::new(
                metadata,
                max_claimable_amount,
                tx_mint_requests_clone,
                pow_secret.as_str(),
                rate_limiter_config,
                &api_keys,
                store,
            );

            // Use select to concurrently:
            // - Run and wait for the faucet (on current thread)
            // - Run and wait for backend server (in a spawned task)
            // - Run and wait for frontend server (in a spawned task, only if set)
            let faucet_future = faucet.run(rx_mint_requests, batch_size);

            let mut tasks = JoinSet::new();
            let mut tasks_ids = HashMap::new();

            let backend_id = tasks.spawn(backend_server.serve(backend_url.clone())).id();
            tasks_ids.insert(backend_id, "backend");

            if let Some(frontend_url) = frontend_url {
                let frontend_id = tasks.spawn(serve_frontend(frontend_url, backend_url)).id();
                tasks_ids.insert(frontend_id, "frontend");
            }

            tokio::select! {
                serve_result = tasks.join_next_with_id() => {
                    let (id, err) = match serve_result.unwrap() {
                        Ok((id, Ok(_))) => (id, Err(anyhow::anyhow!("completed unexpectedly"))),
                        Ok((id, Err(err))) => (id, Err(err)),
                        Err(join_err) => (join_err.id(), Err(join_err).context("failed to join task")),
                    };
                    let component = tasks_ids.get(&id).unwrap_or(&"unknown");
                    err.context(format!("{component} server failed"))
                },
                faucet_result = faucet_future => {
                    // Faucet completed, return its result
                    faucet_result.context("faucet failed")
                },
            }?;
        },

        Command::CreateFaucetAccount {
            output_path,
            token_symbol,
            decimals,
            max_supply,
        } => {
            println!("Generating new faucet account. This may take a few minutes...");

            let current_dir =
                std::env::current_dir().context("failed to open current directory")?;

            let mut rng = ChaCha20Rng::from_seed(rand::random());
            let secret = {
                let auth_seed: [u64; 4] = rng.random();
                let rng_seed = Word::from(auth_seed.map(Felt::new));
                SecretKey::with_rng(&mut RpoRandomCoin::new(rng_seed))
            };

            let symbol = TokenSymbol::try_from(token_symbol.as_str())
                .context("failed to parse token symbol")?;
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

            let account_data = AccountFile::new(account, vec![AuthSecretKey::RpoFalcon512(secret)]);

            let output_path = current_dir.join(output_path);
            account_data.write(&output_path).with_context(|| {
                format!("failed to write account data to file: {}", output_path.display())
            })?;

            println!("Faucet account file successfully created at: {}", output_path.display());
        },

        Command::CreateApiKeys { count: key_count } => {
            let mut rng = ChaCha20Rng::from_seed(rand::random());
            let keys = (0..key_count)
                .map(|_| ApiKey::generate(&mut rng).encode())
                .collect::<Vec<_>>()
                .join(",");
            println!("{keys}");
        },
    }

    Ok(())
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
    use crate::{Cli, run_faucet_command};

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

        // Start the stub node
        tokio::spawn({
            let stub_node_url = stub_node_url.clone();
            async move { serve_stub(&stub_node_url).await.unwrap() }
        });

        let faucet_account_path = temp_dir().join("faucet.mac");

        // Create faucet account
        Box::pin(run_faucet_command(Cli {
            command: crate::Command::CreateFaucetAccount {
                output_path: faucet_account_path.clone(),
                token_symbol: "TEST".to_string(),
                decimals: 6,
                max_supply: 1_000_000_000_000,
            },
        }))
        .await
        .unwrap();

        // Start the faucet connected to the stub
        // Use std::thread to launch faucet - avoids Send requirements
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
                        backend_url: Url::try_from("http://localhost:8000").unwrap(),
                        frontend_url: Some(Url::parse("http://localhost:8080").unwrap()),
                        node_url: stub_node_url,
                        timeout: Duration::from_millis(5000),
                        max_claimable_amount: 1_000_000_000,
                        network: FaucetNetwork::Localhost,
                        api_keys: vec![],
                        pow_secret: "test".to_string(),
                        pow_challenge_lifetime: Duration::from_secs(30),
                        pow_cleanup_interval: Duration::from_secs(1),
                        pow_growth_rate: 1.0,
                        pow_baseline: 12,
                        base_amount: 100_000,
                        faucet_account_path: faucet_account_path.clone(),
                        remote_tx_prover_url: None,
                        open_telemetry: false,
                        store_path: temp_dir().join("test_store.sqlite3"),
                        explorer_url: None,
                        batch_size: 8,
                    },
                }))
                .await
                .unwrap();
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
