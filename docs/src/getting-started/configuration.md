# Configuration

This guide covers all configuration options for the Miden Faucet, including command-line arguments, environment variables, and configuration files.

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
| `--endpoint` | Faucet endpoint (IP:PORT) | - | Yes |
| `--node-url` | Miden node RPC endpoint | - | Yes |
| `--account` | Path to faucet account file | - | Yes |
| `--network` | Network configuration | `localhost` | No |
| `--timeout` | RPC request timeout | `5s` | No |
| `--max-claimable-amount` | Max claimable base units per request | `1000000000` | No |
| `--store` | SQLite store path | `faucet_client_store.sqlite3` | No |

### Proof of Work Configuration

| Option | Description | Default | Required |
|--------|-------------|---------|----------|
| `--pow-secret` | Secret for PoW challenges | "" | No |
| `--pow-baseline` | Base PoW difficulty (0-32) | `12` | No |
| `--pow-challenge-lifetime` | Challenge validity duration | `30s` | No |
| `--pow-cleanup-interval` | Cache cleanup interval | `2s` | No |
| `--pow-growth-rate` | Difficulty growth rate | `1` | No |

### Understanding PoW

The Proof of Work system prevents abuse by requiring computational work:

1. **Baseline Difficulty**: Starting difficulty when no requests are pending
2. **Growth Rate**: How quickly difficulty increases with load
3. **Challenge Lifetime**: How long challenges remain valid
4. **Cleanup Interval**: How often expired challenges are removed

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
export MIDEN_FAUCET_NODE_URL=https://rpc.testnet.miden.io:443
export MIDEN_FAUCET_ACCOUNT_PATH=./faucet.mac
export MIDEN_FAUCET_NETWORK=testnet

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
- **Address Display**: mlcl
- **Use Case**: Local development

#### Devnet
```bash
--network devnet
```
- **Explorer URL**: Not available
- **Address Display**: mdev
- **Use Case**: Development testing

#### Testnet
```bash
--network testnet
```
- **Explorer URL**: https://testnet.midenscan.com/
- **Address Display**: mtst
- **Use Case**: Integration testing

### Custom Network
```bash
--network custom
```

- **Explorer URL**: Not available
- **Address Display**: mcst
- **Use Case**: Run your custom network

## Proof of Work Configuration

### Recommended Settings

```bash
--pow-baseline 12 \
--pow-challenge-lifetime 30s \
--pow-growth-rate 1 \
--pow-cleanup-interval 2s
```

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

## Configuration Examples

### Localhost Setup

```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url http://localhost:57291 \
  --account ./faucet.mac \
  --network localhost \
  --pow-baseline 8 \
  --pow-challenge-lifetime 1s
```

