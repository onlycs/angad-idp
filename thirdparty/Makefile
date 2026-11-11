.PHONY: thirdparty

thirdparty: sqlx

sqlx: Makefile
	rm -rf sqlx

	@echo "=== Preparing/patching SQLx"
	git clone https://github.com/launchbadge/sqlx
	cd sqlx && git checkout v0.9.0
	cd sqlx && cat ../sqlx-join-type-inference.patch | git am -3 --no-gpg-sign
