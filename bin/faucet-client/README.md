# miden-faucet-client

Command-line tool for interacting with a live Miden faucet.

## Commands

### `mint`

Requests tokens from a faucet by solving its PoW challenge and minting a public P2ID note.

```bash
miden-faucet-client mint --url <FAUCET_API_URL> --target-account <ACCOUNT_ID> --amount <AMOUNT>
```
