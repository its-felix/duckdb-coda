# DuckDB Coda Extension

This extension attaches a Coda doc as a DuckDB database. Coda tables are exposed as DuckDB tables.

## Usage

```sql
INSTALL coda;
LOAD coda;

CREATE SECRET coda_token (
    TYPE coda,
    TOKEN 'coda-api-token'
);

ATTACH 'doc-id' AS coda_doc (TYPE coda);

SELECT * FROM coda_doc.main."Tasks";
INSERT INTO coda_doc.main."Tasks" ("Task", "Done") VALUES ('Ship extension', false);
UPDATE coda_doc.main."Tasks" SET "Done" = true WHERE "Task" = 'Ship extension';
DELETE FROM coda_doc.main."Tasks" WHERE "Task" = 'Ship extension';
```

When a token is provided to `CREATE SECRET`, the extension validates it immediately with the Coda `whoami` operation.
Secret creation succeeds only when that request returns HTTP 200; the response body is ignored.

You can also pass credentials at attach time:

```sql
ATTACH 'doc-id' AS coda_doc (TYPE coda, TOKEN 'coda-api-token');
```

To read the API token from an environment variable, use `TOKEN_ENV` with the variable's name:

```sql
CREATE SECRET coda_token (
    TYPE coda,
    TOKEN_ENV 'CODA_API_TOKEN'
);

ATTACH 'doc-id' AS coda_doc (TYPE coda, TOKEN_ENV 'CODA_API_TOKEN');
```

The environment variable is read eagerly when `CREATE SECRET` or `ATTACH` runs. For a secret, the resolved value is
stored as the token; the environment variable name is not retained. `TOKEN` and `TOKEN_ENV` cannot be specified
together.

To expose Coda's row metadata as table columns, enable `INCLUDE_ROW_METADATA` when attaching:

```sql
ATTACH 'doc-id' AS coda_doc (
    TYPE coda,
    TOKEN 'coda-api-token',
    INCLUDE_ROW_METADATA true
);

SELECT "Task", createdAt, updatedAt FROM coda_doc.main."Tasks";
```

With this option enabled, every Coda table includes `createdAt` and `updatedAt` columns. Both columns are
`TIMESTAMP WITH TIME ZONE` values and are read-only.

The initial version intentionally exposes only Coda tables. DDL is not supported: the extension does not create, drop,
or alter Coda tables.

## Column Types

The extension requests rich row values and maps scalar Coda column formats to DuckDB as follows:

| Coda format | DuckDB type |
| --- | --- |
| `checkbox` | `BOOLEAN` |
| `text`, `email`, `select` | `VARCHAR` |
| `number`, `percent`, `slider`, `scale` | `DECIMAL(38, 20)` |
| `date` | `DATE` |
| `dateTime` | `TIMESTAMP WITH TIME ZONE` |
| `time` | `TIME` |
| `duration` | `INTERVAL` |
| `currency` | `STRUCT(currency VARCHAR, amount DECIMAL(38, 20))` |
| `image` | `STRUCT(name VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)` |
| `person` | `STRUCT(name VARCHAR, email VARCHAR)` |
| `link` | `STRUCT(name VARCHAR, url VARCHAR)` |
| `lookup` | `STRUCT(name VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)` |

Array-valued columns use the same mapping for their elements and expose a DuckDB array type; for example, an array
`duration` column becomes `INTERVAL[]`. A scalar `select` exposes its selected value as `VARCHAR`. Unsupported scalar
formats map to `JSON`, and arrays of unsupported values become `JSON[]`.

## Repository Layout

- `src/` contains the Rust extension implementation, C ABI exports, API parsing, and Coda/Superhuman Docs behavior.
- `src/cpp/` contains the DuckDB C++ storage extension plumbing.
- `src/include/` contains the public C ABI and C++ bridge headers.
- `test/sql/` contains DuckDB sqllogictest files loaded through `extension_config.cmake`.
- `Cargo.toml` builds the Rust static library linked by the DuckDB C++ extension.

## Testing

```sh
cargo test
make release
make test
```

`cargo test` runs the Rust unit tests for the ABI implementation, request body generation, and response parsing. The
DuckDB build invokes Cargo to produce a Rust static library and links it into the loadable extension.

The DuckDB extension Makefile targets remain available:

```sh
make verify
make test_coda_http_mock
make test_coda_http_real
```

`make verify` runs the Rust tests and a release build. `make test_coda_http_mock` runs the Rust tests plus ignored
DuckDB integration tests against a local mock HTTP server through `API_BASE`. It expects
`build/release/duckdb` and `build/release/extension/coda/coda.duckdb_extension` to exist; run `make release` first
when those binaries are missing or stale. `make test_coda_http_real` runs a release build plus the ignored live Coda API
integration test.

The real Coda integration tests read `CODA_TEST_API_TOKEN`, `CODA_TEST_DOC_ID`, and `CODA_TEST_WIDE_TABLE_ID` from the
environment. `make test_coda_http_real` exports a local `.env` file before running the tests, matching the same
environment variables provided by CI. The smoke-test harness creates a temporary per-run page in the configured doc,
creates test table content on the page, and deletes the page afterwards.

The public API cannot create or change column formats, so the rich-value integration test uses a persistent wide-table
fixture identified by `CODA_TEST_WIDE_TABLE_ID`. It must contain one populated row and scalar columns named `Checkbox`,
`Text`, `Email`, `Select`, `Number`, `Percent`, `Slider`, `Scale`, `Date`, `DateTime`, `Time`, `Duration`, `Currency`,
`Image`, `Person`, `Hyperlink`, `Lookup`, and `Other`, with the corresponding Coda formats. `Other` must be a Canvas
column so the JSON fallback is covered. It must also contain a populated array-valued `duration` column named
`Durations`. The test validates both the API-reported formats and the resulting DuckDB schema and values.

## Notes

- Coda row IDs are surfaced internally as DuckDB's virtual `rowid` and are used for updates and deletes.
- Coda row metadata is omitted by default. Use `INCLUDE_ROW_METADATA true` on `ATTACH` to include `createdAt` and
  `updatedAt` columns.
- Coda writes are asynchronous. DML reports rows accepted by the Coda API, not rows fully materialized in the doc.
- Explicit DuckDB transactions are not supported for attached Coda databases. Use autocommit statements so the extension
  does not imply rollback semantics that Coda cannot provide.
- Unsupported Coda values use DuckDB's `JSON` type; all array-valued columns preserve their mapped inner type.
- API request construction uses the `superhuman-docs` Rust SDK at tag `v0.2.0`.
