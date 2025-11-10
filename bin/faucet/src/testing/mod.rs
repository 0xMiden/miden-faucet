pub mod stub_rpc_api;

// INTEGRATION TESTS
// ================================================================================================

use std::env::temp_dir;
use std::process::Stdio;
use std::str::FromStr;
use std::time::{Duration, Instant};

use fantoccini::ClientBuilder;
use miden_client::account::{AccountId, Address, NetworkId};
use serde_json::{Map, json};
use tokio::io::AsyncBufReadExt;
use tokio::net::TcpListener;
use tokio::time::sleep;
use url::Url;
use uuid::Uuid;

use crate::network::FaucetNetwork;
use crate::testing::stub_rpc_api::serve_stub;
use crate::{Cli, ClientConfig, run_faucet_command};

/// This test starts a stub node, a faucet connected to the stub node, and a chromedriver
/// to test the faucet website. It then loads the website, mints tokens, and checks that all the
/// requests returned status 200.
#[tokio::test]
async fn frontend_mint_tokens() {
    let stub_node_url = run_stub_node().await;
    let website_url = run_faucet_server(stub_node_url).await;
    let client = start_fantoccini_client().await;

    // Open the website
    client.goto(website_url.as_str()).await.unwrap();

    let title = client.title().await.unwrap();
    assert_eq!(title, "Miden Faucet");

    let network_id = NetworkId::Testnet;
    let account_id = AccountId::try_from(0).unwrap();
    let address = Address::new(account_id);
    let address_bech32 = address.encode(network_id);

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

// TESTING HELPERS
// ---------------------------------------------------------------------------------------------

pub async fn run_stub_node() -> Url {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listener_addr = listener.local_addr().unwrap();
    let stub_node_url = Url::from_str(&format!("http://{listener_addr}")).unwrap();
    tokio::spawn({
        let stub_node_url = stub_node_url.clone();
        async move { serve_stub(&stub_node_url).await.unwrap() }
    });
    stub_node_url
}

async fn run_faucet_server(stub_node_url: Url) -> String {
    let config = ClientConfig {
        node_url: Some(stub_node_url.clone()),
        timeout: Duration::from_millis(5000),
        network: FaucetNetwork::Localhost,
        store_path: temp_dir().join(format!("{}.sqlite3", Uuid::new_v4())),
        remote_tx_prover_url: None,
    };

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

    let api_url = "http://localhost:8000";
    let frontend_url = "http://localhost:8080";

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
                    config,
                    api_url: Url::try_from(api_url).unwrap(),
                    frontend_url: Some(Url::parse(frontend_url).unwrap()),
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
    let api_url = Url::parse(api_url).unwrap();
    let addrs = api_url.socket_addrs(|| None).unwrap();
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

    frontend_url.to_string()
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
    tokio::spawn(async move { chromedriver.wait().await.expect("chromedriver process failed") });
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
