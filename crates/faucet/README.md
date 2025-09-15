# Miden Faucet lib

The Miden faucet library is a Rust library that can be integrated into projects, allowing developers to run the core faucet functionality.

The library provides a `Faucet` struct that can be run in a thread and receive minting requests through a channel. It uses `miden-client` as dependency to create, execute, prove transactions, and then submit them to the node.

## License

This project is [MIT licensed](../../LICENSE).
