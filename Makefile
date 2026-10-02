SHELL := bash

.PHONY: all wasm grpc fmt palette postinstall

all: build

transit-wasm:
	@echo "=== Building transit for wasm32"
	cd src-transit && cargo build -p transit-proto --release --features client --target wasm32-unknown-unknown

transit-native:
	@echo "=== Building transit for native"
	cd src-transit && cargo build -p transit-proto --release --features server,client,uniffi

transit: transit-wasm transit-native

ldap: transit-native
	@echo "=== Cleaning bindings"
	find src-ldap/transit -mindepth 1 -maxdepth 1 ! -name 'go.mod' ! -name '.gitignore' -exec rm -rf -- {} +
	@echo "=== Generating bindings"
	cd src-transit && uniffi-bindgen-go target/release/libtransit_proto.so -o ../src-ldap/transit -c ../src-ldap/uniffi.toml
	@echo "=== Building"
	cd src-ldap && go mod tidy
	cd src-ldap && \
		LD_LIBRARY_PATH="$(LD_LIBRARY_PATH):$(PWD)/src-transit/target/release" \
		CGO_LDFLAGS="-ltransit_proto -L$(PWD)/src-transit/target/release -lm -ldl" \
		CGO_ENABLED=1 \
		go build

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
