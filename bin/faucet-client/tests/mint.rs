use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use miden_client::account::AccountId;
use miden_client::note::NoteId;
use miden_faucet_client::mint::{GetTokensResponse, MintCmd, PowResponse};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct RecordedRequest {
    account_id: Option<String>,
    amount: Option<u64>,
    is_private_note: Option<String>,
    api_key: Option<String>,
    challenge: Option<String>,
}

#[derive(Clone)]
struct AppState {
    pow_response: PowResponse,
    note_id_hex: String,
    tx_id: String,
    recorded: Arc<Mutex<RecordedRequest>>,
}

#[derive(Deserialize)]
struct PowQuery {
    amount: u64,
    account_id: String,
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct TokensQuery {
    account_id: String,
    is_private_note: String,
    asset_amount: u64,
    challenge: String,
    #[allow(dead_code)]
    nonce: u64,
    api_key: Option<String>,
}

#[tokio::test]
async fn mint_command_requests_public_note() {
    let account_hex = "0xca8203e8e58cf72049b061afca78ce";
    let account_id = AccountId::from_hex(account_hex).unwrap();
    let expected_amount = 123_000;
    let pow_response = PowResponse {
        challenge: "00".repeat(32),
        target: u64::MAX,
    };
    let note_id_hex = format!("0x{}", "00".repeat(32));
    let _note_id = NoteId::try_from_hex(&note_id_hex).expect("hex string should produce a note id");
    let app_state = AppState {
        pow_response,
        note_id_hex,
        tx_id: "0xdeadbeef".to_string(),
        recorded: Arc::new(Mutex::new(RecordedRequest::default())),
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/pow", get(pow_handler))
        .route("/get_tokens", get(tokens_handler))
        .with_state(app_state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let cli = MintCmd::parse_from([
        "mint",
        "--url",
        format!("http://{addr}").as_str(),
        "--account",
        account_id.to_hex().as_str(),
        "--quantity",
        &expected_amount.to_string(),
        "--api-key",
        "test-key",
    ]);

    cli.execute().await.unwrap();

    let recorded = app_state.recorded.lock().await.clone();
    assert_eq!(recorded.account_id, Some(account_id.to_hex()));
    assert_eq!(recorded.amount, Some(expected_amount));
    assert_eq!(recorded.is_private_note.as_deref(), Some("false"));
    assert_eq!(recorded.api_key.as_deref(), Some("test-key"));
    assert_eq!(recorded.challenge, Some("00".repeat(32)));
}

async fn pow_handler(
    State(state): State<AppState>,
    Query(params): Query<PowQuery>,
) -> Json<PowResponse> {
    {
        let mut recorded = state.recorded.lock().await;
        recorded.account_id = Some(params.account_id);
        recorded.amount = Some(params.amount);
        recorded.api_key = params.api_key;
    }
    Json(state.pow_response.clone())
}

async fn tokens_handler(
    State(state): State<AppState>,
    Query(params): Query<TokensQuery>,
) -> Json<GetTokensResponse> {
    {
        let mut recorded = state.recorded.lock().await;
        recorded.account_id = Some(params.account_id.clone());
        recorded.amount = Some(params.asset_amount);
        recorded.is_private_note = Some(params.is_private_note.clone());
        recorded.api_key.clone_from(&params.api_key);
        recorded.challenge = Some(params.challenge);
    }
    Json(GetTokensResponse {
        note_id: state.note_id_hex.clone(),
        tx_id: state.tx_id.clone(),
    })
}
