# Miden Faucet

This crate implements a faucet to be used by the website. The faucet uses `miden-client` to create, execute, prove transactions, and then submits them to the node.

The faucet is supposed to be run in a single thread, and receives minting requests via channels.

## License

This project is [MIT licensed](../../LICENSE).
