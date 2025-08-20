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

## Examples

See [Faucet docs](../../docs/src/api-usage.md) for more info on how to interact with the REST API.

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
  - `asset_amount` (number, required): Requested asset amount (in base units)
  - `challenge` (string, required): The encoded challenge from the `/pow` endpoint
  - `nonce` (number, required): The nonce used to solve the challenge
  - `api_key` (string, optional): API key for authentication
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
- `400`: Bad request
- `429`: Rate limited
- `500`: Server error

Error responses include a `message` field with details about the error.

## License

This project is [MIT licensed](../../LICENSE).
