# This file is included by DuckDB's build system. It specifies which extension to load.

duckdb_extension_load(coda
    SOURCE_DIR ${CMAKE_CURRENT_LIST_DIR}
    EXTENSION_VERSION dev
    LOAD_TESTS
)

# The Coda client uses DuckDB's HTTP abstraction; httpfs supplies the full HTTP method implementation.
duckdb_extension_load(httpfs
    GIT_URL https://github.com/duckdb/duckdb-httpfs
    GIT_TAG 53c5b032f6c368cfcc1a1ac3819118e86d3286a6
    APPLY_PATCHES
)
