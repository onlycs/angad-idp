SHELL := bash

.PHONY: all wasm grpc fmt palette postinstall

all: build

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
