.DEFAULT_GOAL := help

ARGS ?=

.PHONY: help build run fmt clippy check test install clean

help:
	@printf "SkillSlash development commands\n\n"
	@printf "Build:\n"
	@printf "  make build        - compile debug binary\n"
	@printf "  make run ARGS=... - run CLI (e.g. ARGS='paths')\n\n"
	@printf "Quality:\n"
	@printf "  make fmt          - format code (cargo fmt)\n"
	@printf "  make clippy       - run clippy\n"
	@printf "  make check        - run fmt + clippy\n\n"
	@printf "Tests:\n"
	@printf "  make test         - run test suite\n\n"
	@printf "Install:\n"
	@printf "  make install      - install local dev build\n\n"
	@printf "Utilities:\n"
	@printf "  make clean        - remove build artifacts\n"

build:
	cargo build

run:
	cargo run -- $(ARGS)

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt clippy

test:
	cargo test

install:
	cargo install --path .

clean:
	cargo clean
