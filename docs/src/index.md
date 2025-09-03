# Miden Faucet

The Miden Faucet is a token distribution service that provides developers and users with test tokens for the Miden blockchain without cost.

It implements proof-of-work challenges to ensure fair distribution of tokens, balancing accessibility for legitimate users while preventing malicious actors from draining the token supply.

It contains a library and a binary CLI.

### Miden faucet library

The Miden faucet library is a Rust library that can be integrated into projects, allowing developers to run the core faucet functionality.

The library provides a `Faucet` struct that can be run in a thread and receive minting requests through a channel.

### Miden faucet CLI

The Miden faucet also includes a command-line interface (CLI) that allows to run the faucet behind a REST API and serves a frontend to interact with it.

More information about the CLI can be found in the [CLI reference](./getting-started/cli.md).

More information about the REST API can be found in the [API reference](./rest-api.md).
