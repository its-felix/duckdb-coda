from .glue import attach_sql, run_duckdb, sql_ident, sql_quote, table_sql


def run_success_case(duckdb, extension, fixture):
    table = table_sql(fixture)
    sql = f"""
{attach_sql(fixture)}
SELECT {sql_ident('Name')}, {sql_ident('Done')}, {sql_ident('Amount')} FROM {table} ORDER BY {sql_ident('Name')};
INSERT INTO {table} ({sql_ident('Name')}, {sql_ident('Done')}, {sql_ident('Amount')})
VALUES ('Gamma', false, 3.5);
UPDATE {table} SET {sql_ident('Done')} = false, {sql_ident('Amount')} = 4.5 WHERE {sql_ident('Name')} = 'Alpha';
DELETE FROM {table} WHERE {sql_ident('Name')} = 'Beta';
"""
    result = run_duckdb(duckdb, extension, sql)
    if "Alpha" not in result.stdout or "Beta" not in result.stdout:
        raise AssertionError(f"expected SELECT output to include seed rows, got:\n{result.stdout}")


def run_metadata_case(duckdb, extension, fixture):
    table = table_sql(fixture)
    sql = f"""
{attach_sql(fixture, include_row_metadata=True)}
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_catalog = 'coda_doc'
  AND table_schema = 'main'
  AND table_name = '{sql_quote(fixture.table_name)}'
  AND column_name IN ('createdAt', 'updatedAt')
ORDER BY column_name;
SELECT
    {sql_ident('Name')},
    typeof(createdAt),
    typeof(updatedAt),
    createdAt IS NOT NULL,
    updatedAt IS NOT NULL
FROM {table}
WHERE {sql_ident('Name')} = 'Alpha';
"""
    result = run_duckdb(duckdb, extension, sql)
    expected_lines = [
        "createdAt,TIMESTAMP WITH TIME ZONE",
        "updatedAt,TIMESTAMP WITH TIME ZONE",
        "Alpha,TIMESTAMP WITH TIME ZONE,TIMESTAMP WITH TIME ZONE,true,true",
    ]
    for line in expected_lines:
        if line not in result.stdout:
            raise AssertionError(f"expected metadata output line {line!r}, got:\n{result.stdout}")
