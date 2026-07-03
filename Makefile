PG_VERSION ?= pg16
WEB_PM ?= bun

.PHONY: dev install-extension db-start db-stop web-install web-dev test test-sql lint build-web

dev: install-extension db-start web-dev

install-extension:
	cargo pgrx install

db-start:
	cargo pgrx start $(PG_VERSION)

db-stop:
	cargo pgrx stop $(PG_VERSION)

web-install:
	cd web && $(WEB_PM) install

web-dev:
	cd web && $(WEB_PM) run dev

test:
	cargo test

test-sql:
	cargo pgrx test $(PG_VERSION)

lint:
	cd web && $(WEB_PM) run lint

build-web:
	cd web && $(WEB_PM) run build
