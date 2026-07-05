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

You can also pass credentials at attach time:

```sql
ATTACH 'doc-id' AS coda_doc (TYPE coda, TOKEN 'coda-api-token');
```

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

## Testing

```sh
make test_debug T=test/sql/coda_offline.test
make test_coda_http_mock
make test_coda_http_real
```

The HTTP mock test starts a local Coda-like server and verifies request paths, query parameters, auth headers, read
responses, DML request bodies, and non-crashing error handling for bad HTTP/JSON responses.

The real Coda integration test reads `CODA_TEST_API_TOKEN` from the environment or local `.env`. Set
`CODA_TEST_DOC_ID` when the token is restricted to a specific doc; the harness creates a temporary per-run page in that
doc, creates test table content on the page, and deletes the page afterwards.

## Notes

- Coda row IDs are surfaced internally as DuckDB's virtual `rowid` and are used for updates and deletes.
- Coda row metadata is omitted by default. Use `INCLUDE_ROW_METADATA true` on `ATTACH` to include `createdAt` and
  `updatedAt` columns.
- Coda writes are asynchronous. DML reports rows accepted by the Coda API, not rows fully materialized in the doc.
- Complex Coda values are represented as JSON text when they cannot be losslessly mapped to a scalar DuckDB type.
- The extension depends on DuckDB's `httpfs` extension for HTTP transport.
