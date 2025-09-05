use base64::Engine;
use base64::engine::general_purpose;
use sha2::{Digest, Sha256};

async fn request_challenge(
    base_url: &str,
    account_address: &str,
) -> anyhow::Result<serde_json::Value> {
    let url = format!("{base_url}/pow?account_id={account_address}");
    let response = reqwest::get(&url).await?.error_for_status()?;
    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    Ok(json)
}

fn solve_challenge(challenge: &str, target: u64) -> u64 {
    let mut found_nonce = None;
    for nonce in 0..u64::MAX {
        // Create SHA3-256 hash
        let mut hasher = Sha256::new();
        hasher.update(challenge.as_bytes());
        hasher.update(nonce.to_be_bytes());
        let hash = hasher.finalize();

        // Take the first 8 bytes and interpret as big-endian u64
        let number = u64::from_be_bytes(hash[..8].try_into().unwrap());

        // Check if hash number is less than target
        if number < target {
            found_nonce = Some(nonce);
            break;
        }
    }
    found_nonce.expect("No valid nonce found")
}

async fn request_tokens(
    base_url: &str,
    account_address: &str,
    challenge: &str,
    nonce: u64,
    asset_amount: u64,
    is_private_note: bool,
) -> anyhow::Result<serde_json::Value> {
    let params = [
        ("account_id", account_address),
        ("is_private_note", if is_private_note { "true" } else { "false" }),
        ("asset_amount", &asset_amount.to_string()),
        ("challenge", challenge),
        ("nonce", &nonce.to_string()),
    ]
    .iter()
    .map(|(key, value)| format!("{key}={value}"))
    .collect::<Vec<_>>()
    .join("&");
    let url = format!("{base_url}/get_tokens?{params}");
    let response = reqwest::get(&url).await?.error_for_status()?;
    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    Ok(json)
}

async fn request_note(base_url: &str, note_id: &str) -> anyhow::Result<Vec<u8>> {
    let url = format!("{base_url}/get_note?note_id={note_id}");
    let response = reqwest::get(&url).await?.error_for_status()?;
    let text = response.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    // Decode base64
    let decoded_bytes =
        general_purpose::STANDARD.decode(json["data_base64"].as_str().unwrap()).unwrap();

    Ok(decoded_bytes)
}

#[tokio::main]
async fn main() {
    // This example assumes you have the faucet running on http://localhost:8080
    let account_address = "mlcl1qq8mcy8pdvl0cgqfkjzf8efjjsnlzf7q";
    let asset_amount = 100;
    let is_private_note = true;
    let url = "http://localhost:8080";

    // Step 1: request challenge
    let challenge_response = request_challenge(url, account_address).await.unwrap();
    let challenge = challenge_response["challenge"].as_str().unwrap();
    let target = challenge_response["target"].as_u64().unwrap();

    // Step 2: solve challenge
    let nonce = solve_challenge(challenge, target);

    // Step 3: request tokens
    let result =
        request_tokens(url, account_address, challenge, nonce, asset_amount, is_private_note)
            .await
            .unwrap();
    println!("Token minted successfully:");
    println!("* Transaction ID: {}", result["tx_id"]);
    println!("* Note ID: {}", result["note_id"]);
    println!("* Explorer URL: {:?}", result["explorer_url"]);

    // Step 4: request note - only necessary for private notes
    let note_data = request_note(url, result["note_id"].as_str().unwrap()).await.unwrap();
    std::fs::write("note.mno", &note_data).unwrap();
    println!("Note saved to note.mno");
}
