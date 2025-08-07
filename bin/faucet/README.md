# Miden faucet

This crate contains a binary for running a Miden testnet faucet.

## Running the faucet

1. Install the faucet:
```bash
make install-faucet
```

2. Create faucet account. This will generate authentication keypair and generate and write public faucet account data with its keypair into the file specified in `output-path`:

```bash
miden-faucet create-faucet-account \
  --output-path <path to faucet.mac> \
  --token-symbol MIDEN \
  --decimals 6 \
  --max-supply 100000000000000000
```
> [!TIP]
> This account will not be created on chain yet, creation on chain will happen on the first minting transaction.

3. Start the faucet server:
```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url https://rpc.testnet.miden.io:443 \
  --account <path to faucet.mac>
```

After a few seconds you may go to `http://localhost:8080` and see the faucet UI.


## Faucet security features:
The faucet implements several security measures to prevent abuse:

- **Proof of Work requests**:
  - Users must complete a computational challenge before their request is processed.
  - The challenge difficulty increases with the load. The load is measured by the amount of challenges that were submitted but still haven't expired.
  - **Rate limiting**: if an account submitted a challenge, it can't submit another one until the previous one is expired. The challenge lifetime duration is fixed and set when running the faucet.
  - **API Keys**: the faucet is initialized with a set of API Keys that can be distributed to developers. The difficulty of the challenges requested using the API Key will increase only with the load of that key, it won't be influenced by the overall load of the faucet.

- **Requests batching**:
  - Maximum batch size: 100 requests
  - Requests are processed in batches to optimize performance
  - Failed requests within a batch are handled individually

## Usage

The faucet can be accessed by the HTTP API interactively through the frontend or programmatically by building the requests manually.

### Programmatic API Usage

The faucet provides a REST API. The typical flow to request tokens involves:

1. **Request a Proof-of-Work challenge** from `/pow`

```rust
let base_url = "http://localhost:8080";
let url = format!("{base_url}/pow?account_id=mlcl1qq8mcy8pdvl0cgqfkjzf8efjjsnlzf7q");
let response = reqwest::get(&url).await?.error_for_status()?.text().await?;
let json: serde_json::Value = serde_json::from_str(&response)?;
let challenge: &str = json["challenge"].as_str().unwrap();
let target: u64 = json["target"].as_u64().unwrap();
```

2. **Solve the computational challenge**

```rust
use sha3::{Digest, Sha3_256};

let nonce = {
    let mut found_nonce = None;
    for nonce in 0..u64::MAX {
        // Create SHA3-256 hash
        let mut hasher = Sha3_256::new();
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
};
```

3. **Request tokens** along with your solved challenge to `/get_tokens`

```rust
let base_url = "http://localhost:8080";
let params = format!("account_id=mlcl1qq8mcy8pdvl0cgqfkjzf8efjjsnlzf7q\
    &is_private_note=true\
    &asset_amount=100\
    &challenge={}\
    &nonce={}", challenge, &nonce.to_string());
let url = format!("{base_url}/get_tokens?{params}");
let response = reqwest::get(&url).await?.error_for_status()?;
let text = response.text().await?;
let json: serde_json::Value = serde_json::from_str(&text)?;
```

4. **Request note** to download generated private notes

```rust
use base64::Engine;
use base64::engine::general_purpose;

let base_url = "http://localhost:8080";
let url = format!("{base_url}/get_note?note_id={note_id}");
let response = reqwest::get(&url).await?.error_for_status()?;
let text = response.text().await?;
let json: serde_json::Value = serde_json::from_str(&text)?;

// Decode note with base64 
let note_data = general_purpose::STANDARD
    .decode(json["data_base64"].as_str().unwrap())
    .unwrap();
std::fs::write("note.mno", &note_data).unwrap();
```

#### Example

To see a full working example, check [request_tokens.rs](bin/faucet/examples/request_tokens.rs). The example assumes you have the faucet running on `http://localhost:8080`.

Run it with:
```bash
cargo run --example request_tokens
```

#### API Endpoints Reference

**GET /pow**
- **Purpose**: Request a proof-of-work challenge
- **Query Parameters**:
  - `account_id` (string, required): The account ID requesting the challenge
  - `api_key` (string, optional): API key for authentication
- **Response**: JSON object containing:
  - `challenge` (string): The encoded challenge string in hexadecimal format
  - `target` (number): The target value for the proof-of-work challenge. A solution is valid if the hash `H(challenge, nonce)` is less than this target. As the hashing function we use `SHA3-256`
  - `timestamp` (number): The timestamp when the challenge was created (seconds since UNIX epoch)

**GET /get_tokens**
- **Purpose**: Request tokens
- **Query Parameters**:
  - `account_id` (string, required): The account ID requesting tokens
  - `is_private_note` (boolean, required): Whether to create a private note
  - `asset_amount` (number, required): Amount of tokens to request
  - `challenge` (string, required): The encoded challenge from the `/pow` endpoint
  - `nonce` (number, required): The nonce used to solve the challenge
  - `api_key` (string, required): API key for authentication
- **Response**: JSON object containing:
  - `tx_id` (string): ID of the created transaction
  - `note_id` (string): ID of the created note
  - `explorer_url` (string): URL to view the transaction in the explorer. Only present if available for the current network.


**GET /get_note**
- **Purpose**: Request a specific note by its ID
- **Query Parameters**:
  - `note_id` (string, required): The ID of the note to retrieve
- **Response**: JSON object containing:
  - `note_id` (string): The ID of the requested note
  - `data_base64` (string): The note data encoded in base64 format. This data should be decoded and saved as a file with `.mno` extension and `application/octet-stream` media type

#### Error Handling

The API returns appropriate HTTP status codes:
- `200`: Success
- `400`: Bad request (invalid parameters or rate limited)
- `500`: Server error

Error responses include a `message` field with details about the error.

## License
This project is [MIT licensed](../../LICENSE).
