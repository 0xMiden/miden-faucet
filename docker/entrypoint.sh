#!/bin/sh
set -e
set -u

# Default to 'start' command if no arguments provided
if [ "$#" -eq 0 ]; then
  set -- start
fi

# Data lives at /faucet by default; override via MIDEN_FAUCET_STORE
: "${MIDEN_FAUCET_STORE:=/faucet/store.sqlite}"

# Ensure store directory exists
STORE_DIR="$(dirname "${MIDEN_FAUCET_STORE}")"
mkdir -p "${STORE_DIR}"

cd "${STORE_DIR}" || exit 1
exec miden-faucet "$@"
