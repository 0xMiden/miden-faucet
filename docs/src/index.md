# Miden Faucet

The Miden faucet contains a library and a binary CLI.

### Miden faucet library

The Miden faucet library is a Rust library that can be integrated into projects, allowing developers to run the core faucet functionality.

The library provides a `Faucet` struct that can be run in a thread and receive minting requests through a channel.

### Miden faucet CLI

The Miden faucet also includes a command-line interface (CLI) that allows to run the faucet behind a REST API and serves a frontend to interact with it.

More information about the CLI can be found in the [CLI reference](./getting-started/cli.md).

More information about the REST API can be found in the [API reference](./rest-api.md).
