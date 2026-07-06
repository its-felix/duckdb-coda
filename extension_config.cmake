# This file is included by DuckDB's build system. It specifies which extension to load.

# Extension from this repo
duckdb_extension_load(coda
    SOURCE_DIR ${CMAKE_CURRENT_LIST_DIR}
    LOAD_TESTS
)

# The Coda client uses DuckDB's HTTP abstraction; httpfs supplies the full HTTP method implementation.
duckdb_extension_load(httpfs
    LOAD_TESTS
    GIT_URL https://github.com/duckdb/duckdb-httpfs
    GIT_TAG 3fb3fc987ca8823979912f35d4acc8b3537b77c7
)
