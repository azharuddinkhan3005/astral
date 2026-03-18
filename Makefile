.PHONY: build release test dev-node dev-python clean

build:
	cargo build --workspace
	cd crates/node && napi build
	cd crates/python && maturin develop

release:
	cargo build --workspace --release
	cd crates/node && napi build --release
	cd crates/python && maturin build --release

test:
	cargo test --workspace
	cd crates/node && npm test
	cd crates/python && pytest

dev-node:
	cd crates/node && napi build && npm link

dev-python:
	cd crates/python && maturin develop --uv

clean:
	cargo clean
	cd crates/node && rm -rf *.node dist
	cd crates/python && rm -rf target dist
