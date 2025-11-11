# CLI configuration and usage

This guide shows the available commands and their configuration options to run with the Miden Faucet CLI.

## Available Commands

| Command | Description |
|---------|-------------|
| `init` | Create the faucet account and initialize the client |
| `start` | Start the faucet server |
| `create-api-key` | Generate an API key for authentication |
| `help` | Show help information |

## Configuration Methods

The Miden Faucet can be configured using:

1. **Command-line arguments**
2. **Environment variables**

## Command-Line Arguments

### Basic Configuration

```bash
miden-faucet init \
  --token-symbol <SYMBOL> \
  --decimals <U8> \
  --max-supply <U64> \
  --node-url <URL> \
  --network <NETWORK>
```

```bash
miden-faucet start \
  --api-port <PORT> \
  --frontend-url <URL> \
  --node-url <URL> \
  --network <NETWORK>
```

## `init` Configuration

### Basic Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--token-symbol` | Symbol of the new token (e.g. "MIDEN", "ETH") | - | Yes (unless `import` is set) |
| `--decimals` | Number of decimals of the new token | - | Yes (unless `import` is set) |
| `--max-supply` | Max supply of the new token (in base units) | - | Yes (unless `import` is set) |
| `--import` | Path to the account file | - | No |
| `--deploy` | Whether to make an empty transaction to deploy the account | `false` | No |
| `--node-url` | Miden node RPC endpoint. If not set, it will be derived from the network | - | No |
| `--timeout` | RPC request timeout | `5s` | No |
| `--network` | Network configuration | `localhost` | No |
| `--store` | SQLite store path | `faucet_client_store.sqlite3` | No |

### Advanced Configuration
| `--remote-tx-prover-url` | Remote transaction prover. Only relevant if `deploy` is set. | - | No |

## `serve` Configuration

### Basic Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--api-port` | Port to bind the API server | - | Yes |
| `--api-public-url` | Public URL to access the faucet API | - | No |
| `--frontend-url` | URL to serve the Frontend API | - | No |
| `--node-url` | Miden node RPC endpoint. If not set, it will be derived from the network | - | No |
| `--network` | Network configuration | `localhost` | No |
| `--timeout` | RPC request timeout | `5s` | No |
| `--max-claimable-amount` | Max claimable base units per request | `1000000000` | No |
| `--store` | SQLite store path | `faucet_client_store.sqlite3` | No |
| `--explorer-url` | Midenscan URL | - | No |
| `--base-amount` | Token amount (in base units) at which the difficulty of the challenge starts to increase. | `100000000` | No |

### Proof of Work Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--pow-secret` | Secret to sign PoW challenges. This should NOT be shared | "" | No |
| `--pow-baseline` | Base PoW difficulty (0-32). It's the starting difficulty when no requests are pending | `12` | No |
| `--pow-challenge-lifetime` | Challenge validity duration, i.e. how long challenges remain valid. This affects the rate limiting, since it works by rejecting new submissions while the previous submitted challenge is still valid | `30s` | No |
| `--pow-cleanup-interval` | Cache cleanup interval, i.e. how often expired challenges are removed | `2s` | No |
| `--pow-growth-rate` | Difficulty growth rate, i.e. how quickly difficulty increases with load. | `0.1` | No |

### Advanced Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--remote-tx-prover-url` | Remote transaction prover | - | No |
| `--api-keys` | Comma-separated API keys | - | No |
| `--enable-otel` | Enable OpenTelemetry | `false` | No |
| `--batch-size` | Maximum number of P2ID notes to create per transaction | `32` | No |

## Environment Variables

All configuration options can be set using environment variables:

```bash
# Faucet Account Configuration
export MIDEN_FAUCET_IMPORT_ACCOUNT_PATH=faucet.mac
export MIDEN_FAUCET_DEPLOY=
export MIDEN_FAUCET_TOKEN_SYMBOL=
export MIDEN_FAUCET_DECIMALS=
export MIDEN_FAUCET_MAX_SUPPLY=

# Faucet Service Configuration
export MIDEN_FAUCET_FRONTEND_URL=http://localhost:8080
export MIDEN_FAUCET_API_PORT=8000
export MIDEN_FAUCET_API_PUBLIC_URL=http://localhost:8000
export MIDEN_FAUCET_MAX_CLAIMABLE_AMOUNT=1000000000
export MIDEN_FAUCET_ENABLE_OTEL=true
export MIDEN_FAUCET_BASE_AMOUNT=100000000

# Network & Node Configuration
export MIDEN_FAUCET_NODE_URL=https://rpc.testnet.miden.io
export MIDEN_FAUCET_NETWORK=testnet
export MIDEN_FAUCET_TIMEOUT=10s
export MIDEN_FAUCET_EXPLORER_URL=https://testnet.midenscan.com
export MIDEN_FAUCET_ACCOUNT_PATH=./faucet.mac

# Faucet Client Configuration
export MIDEN_FAUCET_STORE=faucet_client_store.sqlite3
export MIDEN_FAUCET_REMOTE_TX_PROVER_URL=https://tx-prover.devnet.miden.io
export MIDEN_FAUCET_BATCH_SIZE=32

# Rate Limiting Configuration
export MIDEN_FAUCET_POW_SECRET=your-secret-here
export MIDEN_FAUCET_POW_BASELINE=12
export MIDEN_FAUCET_POW_CHALLENGE_LIFETIME=30s
export MIDEN_FAUCET_POW_CLEANUP_INTERVAL=2s
export MIDEN_FAUCET_POW_GROWTH_RATE=0.1
export MIDEN_FAUCET_API_KEYS=key1,key2,key3
```

## Network Configurations

### Predefined Networks

#### Localhost
```bash
--network localhost
```
- **Explorer URL**: Not available
- **Address Display**: `mlcl`
- **Use Case**: Local development

#### Devnet
```bash
--network devnet
```
- **Explorer URL**: Not available
- **Address Display**: `mdev`
- **Use Case**: Development testing

#### Testnet
```bash
--network testnet
```
- **Explorer URL**: https://testnet.midenscan.com/
- **Address Display**: `mtst`
- **Use Case**: Integration testing

### Custom Network
```bash
--network custom
```

- **Explorer URL**: Not available
- **Address Display**: `mcst`
- **Use Case**: Run your custom network

## API Key Configuration

### Generate API Keys

```bash
miden-faucet create-api-key
```

This generates an API key that can be used for authentication. It is printed to stdout.

### API Key Benefits

- **Rate Limiting**: Separate rate limits per API key
- **Access Control**: Distribute keys to different users/teams

## Store Configuration

### SQLite Store

This is the store that is used by the Miden Client to store all the faucet account state. Default is SQLite:

```bash
--store ./faucet_client_store.sqlite3.sqlite3
```

## Monitoring Configuration

### OpenTelemetry

Enable OpenTelemetry for production monitoring:

```bash
--enable-otel
```

## Configuration Example

```bash
miden-faucet init \
  --token-symbol MIDEN \
  --decimals 6 \
  --max-supply 100000000000000000 \
  --node-url http://localhost:57291

miden-faucet start \
  --frontend-url http://localhost:8080 \
  --api-port 8000 \
  --node-url http://localhost:57291 \
  --network localhost
```

For detailed options, run `miden-faucet [COMMAND] --help`. 
