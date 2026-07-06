PROJ_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

EXT_NAME=coda
EXT_CONFIG=${PROJ_DIR}extension_config.cmake
EXT_FLAGS=-DCMAKE_CXX_STANDARD=17

DEFAULT_TEST_EXTENSION_DEPS=httpfs;json;

include extension-ci-tools/makefiles/duckdb_extension.Makefile
include extension-ci-tools/makefiles/vcpkg.Makefile

.PHONY: test_coda_http_mock
test_coda_http_mock: debug
	python3 test/coda_http_mock.py --backend mock

.PHONY: test_coda_http_real
test_coda_http_real: debug
	python3 test/coda_http_mock.py --backend real --require-real
    