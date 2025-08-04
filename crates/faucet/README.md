# Miden Faucet

This crate implements a faucet to be used by the website. The faucet uses `miden-client` to create, execute, prove transactions, and then submits them to the node.

The faucet is supposed to be run in a single thread, and receives minting requests via channels. In turn, the faucet returns the note_ids for each minting request.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 
