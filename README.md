# Miden faucet

Token faucet application for Miden testnet.

## Documentation

For comprehensive guides, API reference, and examples, see the [Miden Faucet Documentation](https://0xmiden.github.io/miden-faucet).

## Running the faucet

The faucet comes with two CLI tools:
- **miden-faucet**: Runs the faucet, used for initializing and starting the faucet.
- **miden-faucet-client**: Used for interacting with a live faucet, i.e. for requesting tokens from a running faucet.

1. Install both faucet binaries:
```bash
make install-faucet
```

2. Initialize the faucet server. This will generate a new account with the specified token configuration and save the account data to a local SQLite store:

```bash
miden-faucet init \
  --token-symbol MIDEN \
  --decimals 6 \
  --max-supply 100000000000000000 \
  --network testnet
```
> [!TIP]
> This account will not be created on chain yet, creation on chain will happen on the first minting transaction.

> You can also run the legacy alias `miden-faucet` for backwards compatibility; it runs the same `miden-faucet` binary.

3. Start the faucet:
```bash
miden-faucet start \
  --explorer-url https://testnet.midenscan.com \
  --network testnet
```

## Requesting tokens from a live faucet

You can use the `miden-faucet-client` binary to request tokens from any running faucet instance, whether it's your local faucet or the remote testnet faucet:
```bash
miden-faucet-client mint --url <FAUCET_API_URL> --target-account <ACCOUNT_ID> --amount <BASE_UNITS>
```

After a few seconds you may go to `http://localhost:8080` and see the faucet UI.

## Faucet security features:
The faucet implements several security measures to prevent abuse:

- **Proof of Work requests**:
  - Users must complete a computational challenge before their request is processed.
  - The challenge difficulty increases with the load. The load is measured by the amount of challenges that were submitted but still haven't expired.
  - Each challenge is signed with a secret only known by the server. It should NOT be shared.
  - **Rate limiting**: if an account submitted a challenge, it can't submit another one until the previous one is expired. The challenge lifetime duration is fixed and set when running the faucet.
  - **API Keys**: the faucet is initialized with a set of API Keys that can be distributed to developers. The difficulty of the challenges requested using the API Key will increase only with the load of that key, it won't be influenced by the overall load of the faucet.

- **Requests batching**:
  - Maximum batch size: 256 requests
  - Requests are processed in batches to optimize performance
  - Failed requests within a batch are handled individually

## Contributing

Interested in contributing? Check [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

This project is [MIT licensed](./LICENSE).
