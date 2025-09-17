# Changelog

## 0.11.5 (2025-09-18)

- Patched miden-client to 0.11.7 ([#90](https://github.com/0xMiden/miden-faucet/pull/90)).

## 0.11.4 (2025-09-16)

- Reduce faucet batch size to 8 ([#87](https://github.com/0xMiden/miden-faucet/pull/87)).

## 0.11.3 (2025-09-09)

- Fixed display for minted token amount ([#82](https://github.com/0xMiden/miden-faucet/pull/82)).

## 0.11.2 (2025-09-08)

- Refreshed dependencies.

## 0.11.1 (2025-09-02)

- Added `--network` CLI parameter to specify the type of network to which the faucet connects ([#74](https://github.com/0xMiden/miden-faucet/pull/74)).

## 0.11.0 (2025-09-01)

### Changes

- Introduced `miden-faucet-lib` crate ([#10](https://github.com/0xMiden/miden-faucet/pull/10)).
- Integrated miden-client ([#11](https://github.com/0xMiden/miden-faucet/pull/11)).
- Added `/get_note` endpoint ([#19](https://github.com/0xMiden/miden-faucet/pull/19)).
- Redesigned the home frontend ([#20](https://github.com/0xMiden/miden-faucet/pull/20)).
- Redesigned the tokens request flows ([#25](https://github.com/0xMiden/miden-faucet/pull/25)).
- Added faucet supply amounts to the metadata ([#30](https://github.com/0xMiden/miden-faucet/pull/30)).
- Added supply exceeded check ([#31](https://github.com/0xMiden/miden-faucet/pull/31)). 
- Use HTTP 429 status code for rate limited error ([#51](https://github.com/0xMiden/miden-faucet/pull/51)).
- Replace amount options validation for maximum claimable amount ([#52](https://github.com/0xMiden/miden-faucet/pull/52)).
- Added `mdbook` documentation ([#61](https://github.com/0xMiden/miden-faucet/pull/61)).
- Added `--explorer-url` CLI parameter to optionally set the explorer url ([#63](https://github.com/0xMiden/miden-faucet/pull/63)).

## 0.10.0 (2025-07-10)

For previous changes see the changelog in https://github.com/0xMiden/miden-node.
