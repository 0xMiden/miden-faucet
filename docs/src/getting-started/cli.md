# CLI configuration and usage

This guide shows the available commands and their configuration options to run with the Miden Faucet CLI.

## Available Commands

| Command | Description |
|---------|-------------|
| `start` | Start the faucet server |
| `create-faucet-account` | Create a new faucet account |
| `create-api-keys` | Generate API keys for authentication |
| `help` | Show help information |

## Configuration Methods

The Miden Faucet can be configured using:

1. **Command-line arguments**
2. **Environment variables**

## Command-Line Arguments

### Basic Configuration

```bash
miden-faucet start \
  --endpoint <URL> \
  --node-url <URL> \
  --account <PATH> \
  --network <NETWORK>
```

### All Available Options

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--endpoint` | Faucet endpoint | - | Yes |
| `--node-url` | Miden node RPC endpoint | - | Yes |
| `--account` | Path to faucet account file | - | Yes |
| `--network` | Network configuration | `localhost` | No |
| `--timeout` | RPC request timeout | `5s` | No |
| `--max-claimable-amount` | Max claimable base units per request | `1000000000` | No |
| `--store` | SQLite store path | `faucet_client_store.sqlite3` | No |
| `--explorer-url` | Midenscan URL | - | No |

### Proof of Work Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--pow-secret` | Secret to sign PoW challenges. This should NOT be shared | "" | No |
| `--pow-baseline` | Base PoW difficulty (0-32). It's the starting difficulty when no requests are pending | `12` | No |
| `--pow-challenge-lifetime` | Challenge validity duration, i.e. how long challenges remain valid. This affects the rate limiting, since it works by rejecting new submissions while the previous submitted challenge is still valid | `30s` | No |
| `--pow-cleanup-interval` | Cache cleanup interval, i.e. how often expired challenges are removed | `2s` | No |
| `--pow-growth-rate` | Difficulty growth rate, i.e. how quickly difficulty increases with load. When set to 1, the difficulty will roughly double when the number of requests doubles. | `1` | No |

### Advanced Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--remote-tx-prover-url` | Remote transaction prover | - | No |
| `--api-keys` | Comma-separated API keys | - | No |
| `--enable-otel` | Enable OpenTelemetry | `false` | No |

## Environment Variables

All configuration options can be set using environment variables:

```bash
# Basic configuration
export MIDEN_FAUCET_ENDPOINT=http://localhost:8080
export MIDEN_FAUCET_NODE_URL=https://rpc.testnet.miden.io
export MIDEN_FAUCET_ACCOUNT_PATH=./faucet.mac
export MIDEN_FAUCET_NETWORK=testnet
export MIDEN_FAUCET_EXPLORER_URL=https://testnet.midenscan.com

# Proof of Work
export MIDEN_FAUCET_POW_SECRET=your-secret-here
export MIDEN_FAUCET_POW_BASELINE=12
export MIDEN_FAUCET_POW_CHALLENGE_LIFETIME=30s
export MIDEN_FAUCET_POW_GROWTH_RATE=1

# Advanced
export MIDEN_FAUCET_MAX_CLAIMABLE_AMOUNT=1000
export MIDEN_FAUCET_TIMEOUT=10s
export MIDEN_FAUCET_ENABLE_OTEL=true
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
miden-faucet create-api-keys 5
```

This generates 5 API keys that can be used for authentication. They are printed to stdout.

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
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url http://localhost:57291 \
  --account ./faucet.mac \
  --network localhost \
  --pow-baseline 8 \
  --pow-challenge-lifetime 1s
```

For detailed options, run `miden-faucet [COMMAND] --help`. 
