SHELL := bash

.PHONY: help
.PHONY: proto proto-wasm proto-native
.PHONY: ldap wasm api
.PHONY: fmt postinstall

help:
	@echo "Usage: make [target]"
	@echo "Targets:"
	@echo "  help           - Show this help message"
	@echo "  ldap           - Build the go ldap binary"
	@echo "  wasm           - Build WASM package and generate bindings"
	@echo "  api            - Build API"
	@echo "  fmt            - Format everything"

define RUST_SRC
$(shell find $(1)/src -type f -name '*.rs') \
$(1)/Cargo.toml \
$(1)/Cargo.lock
endef

define GO_SRC
$(shell find $(1)/ -type f -name '*.go') \
$(1)/go.mod \
$(1)/go.sum
endef

proto-wasm: $(call RUST_SRC,src-proto)
	@echo "=== Building proto for wasm32"
	cd src-proto && cargo build --release --features client,tokio --target wasm32-unknown-unknown

proto-native: $(call RUST_SRC,src-proto)
	@echo "=== Building proto for native"
	cd src-proto && cargo build --release --features client,uniffi

proto: proto-wasm proto-native

ldap: $(call GO_SRC,src-ldap)
	@echo "=== Cleaning bindings"
	find src-ldap/transit -mindepth 1 -maxdepth 1 ! -name 'go.mod' ! -name '.gitignore' -exec rm -rf -- {} +
	@echo "=== Generating bindings"
	cd src-proto && uniffi-bindgen-go target/release/libidp_proto.so -o ../src-ldap/transit -c ../src-ldap/uniffi.toml
	@echo "=== Building"
	cd src-ldap && go mod tidy && rm -rf out
	cd src-ldap && \
    	CGO_LDFLAGS="$(PWD)/src-proto/target/release/libidp_proto.a -lm -ldl -lpthread" \
    	CGO_ENABLED=1 \
		go build -o out/idp-ldap

wasm: $(call RUST_SRC,src-proto)
	@echo "=== Building WASM package"
	cd src-proto && rm -rf pkg && wasm-pack build --target web --release --out-name libidp --features client,tokio
	@echo "=== Copying package files"
	rm -rf static/wasm
	mkdir -p static
	cp -r src-proto/pkg static/wasm

thirdparty/sqlx:
	@echo "=== Preparing/patching SQLx"
	cd thirdparty && git clone https://github.com/launchbadge/sqlx
	cd thirdparty/sqlx && git checkout v0.9.0
	cd thirdparty/sqlx && cat ../sqlx-join-type-inference.patch | git am -3 --no-gpg-sign

api: thirdparty/sqlx $(call RUST_SRC,src-idp)
	@echo "=== Building the API"
	cd src-idp && cargo build --release

fmt:
	@echo "=== Formatting code"
	bun fmt
	cd src-idp && cargo fmt
	cd src-ldap && go fmt
	cd src-proto && cargo fmt

build: api wasm ldap
	@echo "=== Installing dependencies"
	bun i # will run `make postinstall` (see package.json)

	@echo "=== Building frontend"
	bun generate

precommit:
	cd src-idp && cargo sqlx prepare

postinstall: wasm fmt
