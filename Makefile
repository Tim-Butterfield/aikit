# Development targets for aikit.
#
# REQUIRES GNU MAKE AND A POSIX SHELL. This file uses `?=`, `:=`, `.PHONY`, `.DEFAULT_GOAL`
# and `$(MAKEFILE_LIST)`, and its recipes call grep, awk and `command -v` through pipes.
# None of that is portable to other makes or to cmd.
#
# On Windows a `make` on PATH is frequently Embarcadero's, installed by Delphi, C++Builder
# or RAD Studio. It cannot parse this file and reports "colon expected" against the `.PHONY`
# line below — which plainly has a colon; that parser just has no such special target. Run
# these targets from Git Bash or MSYS2, or skip make entirely: every target is one cargo
# command, listed in the README.
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
