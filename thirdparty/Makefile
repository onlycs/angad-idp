.PHONY: thirdparty

thirdparty: sqlx carbon-components-svelte

sqlx: Makefile sqlx-join-type-inference.patch
	rm -rf sqlx

	@echo "=== Preparing/patching SQLx"
	mkdir sqlx
	cd sqlx && git init . && git remote add origin https://github.com/launchbadge/sqlx
	cd sqlx && git fetch --depth 1 origin 75bc0487eb661da811bb7a3c5d158f1bd463fef4 && git checkout --detach FETCH_HEAD
	cd sqlx && cat ../sqlx-join-type-inference.patch | git am -3 --no-gpg-sign

carbon-components-svelte: Makefile carbon-bugfixes.patch
	rm -rf carbon-components-svelte
	mkdir carbon-components-svelte
	cd carbon-components-svelte && git init . && git remote add origin https://github.com/carbon-design-system/carbon-components-svelte
	cd carbon-components-svelte && git fetch --depth 1 origin 40d977497d81c21e9e4002950f13f84763552681 && git checkout --detach FETCH_HEAD
	cd carbon-components-svelte && cat ../carbon-bugfixes.patch | git apply -3 -
	cd carbon-components-svelte && bun i && bun run build:docs && bun run build:css:themes
	cd carbon-components-svelte && bun pm pack
