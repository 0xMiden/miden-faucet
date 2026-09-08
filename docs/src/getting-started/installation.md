# Installation

We provide Docker images for official releases for the Faucet software. Alternatively, it also can be installed from source on most systems using the Rust package manager `cargo`.

## Docker image

Official Docker images are published to `ghcr.io/0xmiden/miden-faucet` for every release.

```sh
docker pull ghcr.io/0xmiden/miden-faucet:<version>
```

See the [README](https://github.com/0xMiden/faucet#readme) for the available configuration options and an example
invocation.

## Install using `cargo`

Install Rust version **1.89** or greater using the official Rust installation
[instructions](https://www.rust-lang.org/tools/install).

Depending on the platform, you may need to install additional libraries. For example, on Ubuntu 22.04 the following
command ensures that all required libraries are installed.

```sh
sudo apt install llvm clang bindgen pkg-config libssl-dev libsqlite3-dev
```

Install the latest faucet binary:

```sh
cargo install miden-faucet --locked
cargo install miden-faucet-client --locked
```

This will install the latest official version of the faucet. You can install a specific version `x.y.z` using

```sh
cargo install miden-faucet --locked --version x.y.z
cargo install miden-faucet-client --locked --version x.y.z
```

You can also use `cargo` to compile the node from the source code if for some reason you need a specific git revision.
Note that since these aren't official releases we cannot provide much support for any issues you run into, so consider
this for advanced use only. The incantation is a little different as you'll be targeting our repo instead:

```sh
# Install from a specific branch
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet --branch <branch>
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet-client --branch <branch>

# Install a specific tag
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet --tag <tag>
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet-client --tag <tag>

# Install a specific git revision
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet --rev <git-sha>
cargo install --locked --git https://github.com/0xMiden/faucet miden-faucet-client --rev <git-sha>

> Use `miden-faucet` to initialize/start the faucet service, and `miden-faucet-client` to mint from a running faucet.
```

More information on the various `cargo install` options can be found
[here](https://doc.rust-lang.org/cargo/commands/cargo-install.html#install-options).

## Updating

Updating the faucet to a new version is as simply as re-running the install process.

If the node version is updated, you may encounter an error like this:
```bash
Error: faucet failed

Caused by:
    0: transaction executor error
    1: failed to execute transaction kernel program:
         × advice provider error at clock cycle 152
         ╰─▶   × value for key 0x85dee386c7e023b13a5cf16def1c421c57f76c9135a665bdd5b547d6a54d1b15 not present in the advice map
```

This is a common error that occurs when the Miden network undergoes updates during its active development phase. The error happens because the local client store contains data that's incompatible with the updated node version.

To resolve this issue, delete the `faucet_client_store.sqlite3` file and restart the faucet. This will force the client to re-sync with the updated network state.
