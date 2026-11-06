.PHONY: thirdparty

thirdparty: sqlx carbon-components-svelte

sqlx:
	@echo "=== Preparing/patching SQLx"
	git clone https://github.com/launchbadge/sqlx
	cd sqlx && git checkout v0.9.0
	cd sqlx && cat ../sqlx-join-type-inference.patch | git am -3 --no-gpg-sign

carbon-components-svelte: Makefile
	rm -rf carbon-components-svelte
	mkdir carbon-components-svelte
	cd carbon-components-svelte && git init . && git remote add origin https://github.com/carbon-design-system/carbon-components-svelte
	cd carbon-components-svelte && git fetch --depth 1 origin 0239bd5fb9a4ff29f717207a7a0a9ffb10d33796 && git checkout --detach FETCH_HEAD
	cd carbon-components-svelte && bun i && bun run build:docs && bun run build:css:themes
	cd carbon-components-svelte && bun pm pack
