SHELL := bash

.PHONY: help
.PHONY: transit transit-wasm transit-native
.PHONY: ldap wasm api
.PHONY: fmt postinstall

help:
	@echo "Usage: make [target]"
	@echo "Targets:"
	@echo "  help           - Show this help message"
	@echo "  transit        - Build transit for wasm32 and native"
	@echo "  transit-wasm   - Build transit for wasm32"
	@echo "  transit-native - Build transit for native"
	@echo "  ldap           - Build the go ldap binary"
	@echo "  wasm           - Build WASM package and generate bindings"
	@echo "  api            - Build API"
	@echo "  fmt            - Format everything"

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
	cd src-wasm && rm -rf pkg && wasm-pack build --target web --release --out-name libidp
	@echo "=== Copying package files"
	rm -rf static/wasm
	mkdir -p static
	cp -r src-wasm/pkg static/wasm
	@echo "=== Patching workerHelpers.js files"
	sed -i 's|\.\./\.\./\.\.|../../../libidp.js|g' static/wasm/snippets/*/src/workerHelpers.js

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

postinstall: wasm
