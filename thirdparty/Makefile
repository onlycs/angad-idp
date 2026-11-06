.PHONY: thirdparty

thirdparty: sqlx carbon-components-svelte

sqlx:
	@echo "=== Preparing/patching SQLx"
	git clone https://github.com/launchbadge/sqlx
	cd sqlx && git checkout v0.9.0
	cd sqlx && cat ../sqlx-join-type-inference.patch | git am -3 --no-gpg-sign

carbon-components-svelte:
	@echo "=== Preparing carbon-components-svelte from master"
	git clone https://github.com/carbon-design-system/carbon-components-svelte --depth 1
	cd carbon-components-svelte && bun i && bun run build:docs && bun run build:css:themes
	cd carbon-components-svelte && bun pm pack
