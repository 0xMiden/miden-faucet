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

```typescript
const baseUrl = 'http://localhost:8080';
const accountId = 'mlcl1qq8mcy8pdvl0cgqfkjzf8efjjsnlzf7q';

const powUrl = new URL('/pow', baseUrl);
powUrl.searchParams.set('account_id', accountId);
const powResp = await fetch(powUrl);
if (!powResp.ok) throw new Error(`PoW error: ${powResp.status} ${await powResp.text()}`);
const powJson: any = await powResp.json();
const challenge: string = powJson.challenge;
const target: bigint = BigInt(powJson.target);
```

2. **Solve the computational challenge**

```typescript
// Dependencies: npm i @noble/hashes
import { sha3_256 } from '@noble/hashes/sha3';

let nonce = 0;
while (true) {
    nonce = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);

    try {
        // Compute hash using SHA3-256 with the challenge and nonce
        let hash = sha3_256.create();
        hash.update(challenge); // Use the hex-encoded challenge string directly

        // Convert nonce to 8-byte big-endian format to match backend
        const nonceBytes = new ArrayBuffer(8);
        const nonceView = new DataView(nonceBytes);
        nonceView.setBigUint64(0, BigInt(nonce), false); // false = big-endian
        const nonceByteArray = new Uint8Array(nonceBytes);
        hash.update(nonceByteArray);

        // Take the first 8 bytes of the hash and parse them as u64 in big-endian
        const hashBytes: Uint8Array = hash.digest().slice(0, 8);
        let digest = BigInt('0x' + Array.from(hashBytes).map(b => b.toString(16).padStart(2, '0')).join(''));

        // Check if the hash is less than the target
        if (digest < target) {
            console.log('Found nonce:', nonce);
            return nonce;
        }
    } catch (error: any) {
        throw new Error('Failed to compute hash: ' + error.message);
    }
}
```

3. **Request tokens** along with your solved challenge to `/get_tokens`

```typescript
const params = new URLSearchParams({
    account_id: accountId,
    is_private_note: 'true',
    asset_amount: '100',
    challenge: challenge,
    nonce: nonce.toString()
});

const response = await fetch(`${baseUrl}/get_tokens?${params}`);
if (!response.ok) throw new Error(`Get tokens error: ${response.status} ${await response.text()}`);

const text = await response.text();
const json = JSON.parse(text);
const noteId = json.note_id;
const txId = json.tx_id;
const explorerUrl = json.explorer_url;
```

4. **Request note** to download generated private notes

```typescript
const response = await fetch(`${baseUrl}/get_note?note_id=${noteId}`);
if (!response.ok) throw new Error(`Get note error: ${response.status} ${await response.text()}`);

const text = await response.text();
const json = JSON.parse(text);

// Decode note with base64
const noteData = Buffer.from(json.data_base64, 'base64');

fs.writeFileSync('note.mno', noteData);
```

#### Example

Check out the complete working examples below. Make sure the faucet is running at `http://localhost:8080` before using them.
- Rust: [`examples/rust/request_tokens.rs`](examples/request_tokens.rs)
- TypeScript: [`examples/typescript/request_tokens.ts`](examples/ts/request_tokens.ts)

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
