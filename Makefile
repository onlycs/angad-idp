SHELL := bash

.PHONY: all wasm grpc fmt palette postinstall

all: build

transit:
	@echo "=== Building transit for wasm32"
	cd src-transit && cargo build -p transit-core --release --features client --target wasm32-unknown-unknown

	@echo "=== Building server for non-wasm"
	cd src-transit && cargo build -p transit-core --release --features server

	@echo "=== Building client for non-wasm"
	cd src-transit && cargo build -p transit-core --release --features client

	@echo "=== Building protocol for wasm32"
	cd src-transit && cargo build -p transit-proto --release --target wasm32-unknown-unknown

	@echo "=== Building protocol for non-wasm"
	cd src-transit && cargo build -p transit-proto --release

wasm:
	@echo "=== Building WASM package"
	cd src-crypto && rm -rf pkg && wasm-pack build --target web --release --out-name crypto
	@echo "=== Copying package files"
	rm -rf static/wasm
	mkdir -p static
	cp -r src-crypto/pkg static/wasm
	@echo "=== Patching workerHelpers.js files"
	sed -i 's|\.\./\.\./\.\.|../../../crypto.js|g' static/wasm/snippets/*/src/workerHelpers.js

grpc:
	@echo "=== TODO (grpc)"

fmt:
	@echo "=== Formatting code"
	bun fmt
	cd src-api && cargo fmt
	cd src-crypto && cargo fmt

build:
	@echo "=== Building the API"
	cd src-api && cargo build --release

	@echo "=== Installing dependencies"
	bun i # will run `make postinstall` (see package.json)

	@echo "=== Building frontend"
	bun generate

postinstall: grpc wasm
