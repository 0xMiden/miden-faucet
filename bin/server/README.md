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

## API Endpoints

The server exposes the following endpoints:

- `GET /` - Request index page html
- `GET /index.js` - Request index page js code
- `GET /index.css` - Request index page style
- `GET /background.png` - Request background image
- `GET /favicon.ico` - Request favicon image
- `GET /get_metadata` - Request faucet metadata
- `GET /pow` - Request a proof-of-work challenge. **Query params:**  
  - `account_id`: The recipient's account address in hex or bech32 format.
  - `api_key`: Optional API key distributed to developers to reduce PoW difficulty.
- `GET /get_tokens` - Request tokens. **Query params:**  
  - `account_id`: The recipient's account address in hex or bech32 format.
  - `is_private_note`: Whether to send tokens as a private note.
  - `asset_amount`: The amount of tokens to send.
  - `challenge`: The PoW challenge encoded as string.
  - `nonce`: The valid nonce that solves the challenge.
  - `api_key`: Optional API key. It must match the one used for issuing the PoW challenge.

## License
This project is [MIT licensed](../../LICENSE).


## License
This project is [MIT licensed](../../LICENSE).
