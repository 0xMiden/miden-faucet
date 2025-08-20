# Quick Start

Get the Miden Faucet running in minutes.

## Prerequisites

- Miden Faucet installed (see [Installation](./installation.md))
- Access to a Miden node (testnet or local)

## Step 1: Create a Faucet Account

```bash
miden-faucet create-faucet-account \
  --output-path ./faucet.mac \
  --token-symbol MIDEN \
  --decimals 6 \
  --max-supply 100000000000000000
```

## Step 2: Start the Faucet Server

```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url https://rpc.testnet.miden.io:443 \
  --account ./faucet.mac \
```

## Step 3: Access the Web Interface

Open `http://localhost:8080` in your browser.

## Step 4: Request Test Tokens

### Via Web Interface

1. Enter your Miden account ID or account bech32 address.
2. Select token amount
3. Choose note type (private or public)
4. Submit request

### Via API

You can also programmatically interact with the REST API to mint tokens. Check out the complete working examples below. Make sure the faucet is running at `http://localhost:8080` before using them.
- [Rust](../examples/rust/request_tokens.rs)
- [TypeScript](../examples/typescript/request_tokens.ts)

## Common Configurations

### Localhost

If you have a Miden Node running locally, you can run the faucet against that node.

```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url http://localhost:57291 \
  --account ./faucet.mac \
  --network localhost
```

### Development

Connect to the node deployed in Miden Devnet.

```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url http://rpc.devnet.miden.io:443 \
  --account ./faucet.mac \
  --network devnet
```

### Testnet

Connect to the node deployed in Miden Testnet.

```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url https://rpc.testnet.miden.io:443 \
  --account ./faucet.mac \
  --network testnet
``` 
