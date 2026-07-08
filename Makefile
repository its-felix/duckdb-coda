PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXT_NAME=coda
EXT_CONFIG=${PROJ_DIR}extension_config.cmake
EXT_FLAGS=-DCMAKE_CXX_STANDARD=17

DEFAULT_TEST_EXTENSION_DEPS=

include extension-ci-tools/makefiles/duckdb_extension.Makefile
include extension-ci-tools/makefiles/vcpkg.Makefile

.PHONY: verify
verify:
	cargo test
	$(MAKE) release

.PHONY: test_coda_http_mock
test_coda_http_mock:
	cargo test
	cargo test duckdb_mock_coda -- --ignored --nocapture

.PHONY: test_coda_http_real
test_coda_http_real:
	$(MAKE) release
	set -a; [ ! -f .env ] || . ./.env; set +a; cargo test real_coda_api -- --ignored --nocapture
