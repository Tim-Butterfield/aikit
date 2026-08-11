# Development targets for aikit.
#
# `make install` puts this working tree's aikit on your PATH, which is how you dogfood a
# change. Note that it REPLACES the aikit you may already be using for other work — run
# `make verify` first, and `make uninstall` to go back to nothing.

CARGO ?= cargo
WINDOWS_TARGET ?= x86_64-pc-windows-msvc

.DEFAULT_GOAL := verify
.PHONY: help build test fmt fmt-check lint lint-windows verify install uninstall clean

help: ## Show this help
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
	  | awk -F':.*?## ' '{ printf "  %-14s %s\n", $$1, $$2 }'

build: ## Debug build
	$(CARGO) build

test: ## Run the whole test suite
	$(CARGO) test

fmt: ## Format the source
	$(CARGO) fmt

fmt-check: ## Fail if the source is not formatted
	$(CARGO) fmt --check

lint: ## Clippy on the host target
	$(CARGO) clippy --all-targets -- -D warnings

lint-windows: ## Typecheck the Windows-only code paths without a Windows machine
	@rustup target list --installed | grep -q '^$(WINDOWS_TARGET)$$' \
	  || rustup target add $(WINDOWS_TARGET)
	$(CARGO) clippy --target $(WINDOWS_TARGET) -- -D warnings

verify: fmt-check lint lint-windows test ## Everything CI would check

install: ## Install this working tree's aikit to ~/.cargo/bin (REPLACES any installed aikit)
	$(CARGO) install --path . --locked --force
	@echo
	@echo "Installed: $$(command -v aikit)"
	@aikit version

uninstall: ## Remove the installed aikit
	$(CARGO) uninstall aikit

clean: ## Remove build artifacts
	$(CARGO) clean
