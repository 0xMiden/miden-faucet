.DEFAULT_GOAL := help

.PHONY: help
help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

# -- variables ------------------------------------------------------------------------------------

WARNINGS=RUSTDOCFLAGS="-D warnings"

# -- linting --------------------------------------------------------------------------------------

.PHONY: clippy
clippy: ## Runs Clippy with configs
	cargo clippy --locked --all-targets --all-features --workspace


.PHONY: fix
fix: ## Runs Fix with configs
	cargo fix --allow-staged --allow-dirty --all-targets --all-features --workspace


.PHONY: format
format: ## Runs Format using nightly toolchain
	cargo +nightly fmt --all


.PHONY: format-check
format-check: ## Runs Format using nightly toolchain but only in check mode
	cargo +nightly fmt --all --check


.PHONY: machete
toml: ## Runs machete to find unused dependencies
	cargo machete


.PHONY: toml
toml: ## Runs Format for all TOML files
	taplo fmt


.PHONY: toml-check
toml-check: ## Runs Format for all TOML files but only in check mode
	taplo fmt --check --verbose

.PHONY: typos-check
typos-check: ## Runs spellchecker
	typos

.PHONY: workspace-check
workspace-check: ## Runs a check that all packages have `lints.workspace = true`
	cargo workspace-lints


.PHONY: lint
lint: typos-check format fix clippy toml workspace-check machete ## Runs all linting tasks at once (Clippy, fixing, formatting, workspace, machete)

# --- docs ----------------------------------------------------------------------------------------

.PHONY: doc
doc: ## Generates & checks documentation
	$(WARNINGS) cargo doc --all-features --keep-going --release --locked

.PHONY: book
book: ## Builds the book & serves documentation site
	mdbook serve --open docs

# --- testing -------------------------------------------------------------------------------------

.PHONY: test
test:  ## Runs all tests
	cargo nextest run --all-features --workspace

# --- checking ------------------------------------------------------------------------------------

.PHONY: check
check: ## Check all targets and features for errors without code generation
	${BUILD_PROTO} cargo check --all-features --all-targets --locked --workspace

# --- installing ----------------------------------------------------------------------------------

.PHONY: install-faucet
install-faucet: ## Installs faucet
	${BUILD_PROTO} cargo install --path bin/faucet --locked

# --- docker --------------------------------------------------------------------------------------

.PHONY: docker-build-faucet
docker-build-faucet: ## Builds the Miden faucet using Docker
	@CREATED=$$(date) && \
	VERSION=$$(cat bin/faucet/Cargo.toml | grep -m 1 '^version' | cut -d '"' -f 2) && \
	COMMIT=$$(git rev-parse HEAD) && \
	docker build --build-arg CREATED="$$CREATED" \
        		 --build-arg VERSION="$$VERSION" \
          		 --build-arg COMMIT="$$COMMIT" \
                 -f bin/faucet/Dockerfile \
                 -t miden-faucet-image .

.PHONY: docker-run-faucet
docker-run-faucet: ## Runs the Miden faucet as a Docker container
	docker volume create miden-db
	docker run --name miden-faucet \
			   -p 8080:8080 \
               -d miden-faucet-image
