# Priority Agent - One-command build & install

PREFIX ?= $(HOME)/.local
FEATURES ?=

.PHONY: all build release install install-cli test clean lint help

all: release

build:
	cargo build --quiet

release:
	cargo build --release --quiet

install:
	./scripts/install.sh --release --prefix $(PREFIX)

install-cli: install

test:
	cargo test --quiet

clean:
	cargo clean

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

help:
	@echo "Priority Agent - Makefile targets"
	@echo ""
	@echo "  make              Build release binary"
	@echo "  make install      Build release + install to ~/.local/bin (chat CLI enabled)"
	@echo "  make install-cli  Alias of make install"
	@echo "  make test         Run all tests"
	@echo "  make clean        Clean build artifacts"
	@echo "  make lint         Run fmt and clippy checks"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=path       Install prefix (default: ~/.local)"
	@echo "  FEATURES=list     Additional cargo features"
