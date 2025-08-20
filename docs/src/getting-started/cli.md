# CLI Commands

This guide shows the available commands to run with the Miden Faucet CLI.

## Available Commands

| Command | Description |
|---------|-------------|
| `start` | Start the faucet server |
| `create-faucet-account` | Create a new faucet account |
| `create-api-keys` | Generate API keys for authentication |
| `help` | Show help information |

## Start Command

Start the faucet server:

```bash
miden-faucet start [OPTIONS]
```

### Required Options

- `--endpoint`: Faucet endpoint (IP:PORT)
- `--node-url`: Miden node RPC endpoint  
- `--account`: Path to faucet account file

### Common Options

- `--max-claimable-amount`: Max token base units per request (default: 1000000000)
- `--pow-baseline`: Base PoW difficulty 0-32 (default: 12)
- `--timeout`: RPC request timeout (default: 5s)

### Examples

**Localhost:**
```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url http://localhost:57291 \
  --account ./faucet.mac
```

**Testnet:**
```bash
miden-faucet start \
  --endpoint http://localhost:8080 \
  --node-url https://rpc.testnet.miden.io:443 \
  --account ./faucet.mac
```

## Create Faucet Account

```bash
miden-faucet create-faucet-account \
  --output-path ./faucet.mac \
  --token-symbol MIDEN \
  --decimals 6 \
  --max-supply 100000000000000000
```

## Create API Keys

```bash
miden-faucet create-api-keys --count 3
```

For detailed options, run `miden-faucet [COMMAND] --help`. 
