SHELL := bash

.PHONY: all wasm grpc fmt typegen postinstall

all: build

wasm:
	@echo "=== Building WASM package"
	cd src-crypto && rm -rf pkg && wasm-pack build --target web --release
	@echo "=== Copying package files"
	rm -rf public/wasm
	cp -r src-crypto/pkg public/wasm
	@echo "=== Patching workerHelpers.js files"
	sed -i 's|\.\./\.\./\.\.|../../../auth_crypto.js|g' public/wasm/snippets/*/src/workerHelpers.js

grpc:
	@echo "=== TODO (grpc)"

fmt:
	@echo "=== Formatting code"
	bun fmt
	cd src-api && cargo fmt
	cd src-crypto && cargo fmt

typegen:
	@echo "=== Generating TypeScript types"
	bunx nuxi prepare

build:
	@echo "=== Building the API"
	cd src-api && cargo build --release

	@echo "=== Installing dependencies"
	bun i # will run `make postinstall` (see package.json)

	@echo "=== Building frontend"
	bun generate

postinstall: grpc wasm fmt typegen
