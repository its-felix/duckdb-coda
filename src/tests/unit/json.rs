use super::*;

#[test]
fn parse_columns_and_rows() {
    let columns = column_list_from_json(
        r#"{"items":[{"id":"c-id","name":"Amount","calculated":true,"format":{"type":"currency","isArray":false}}],"nextPageToken":"next"}"#,
    )
    .unwrap();
    assert_eq!(columns.items.len(), 1);
    assert_eq!(columns.items[0].id, "c-id");
    assert_eq!(
        columns.items[0].duckdb_type,
        "STRUCT(currency VARCHAR, amount DECIMAL(38,20))"
    );

    let rows = rows_from_json(
        r#"{"items":[{"id":"r1","createdAt":"2024-01-01T00:00:00Z","values":{"c1":true,"c2":[1,2],"c3":"plain"}}],"nextSyncToken":"sync"}"#,
    )
    .unwrap();
    assert_eq!(rows.rows.len(), 1);
    assert_eq!(rows.rows[0].cells.len(), 3);
    assert_eq!(rows.next_sync_token, "sync");
}

#[test]
fn documented_column_formats_map_to_duckdb_logical_types() {
    for (format_type, expected) in [
        ("checkbox", "BOOLEAN"),
        ("text", "VARCHAR"),
        ("email", "VARCHAR"),
        ("select", "VARCHAR"),
        ("number", "DECIMAL(38,20)"),
        ("percent", "DECIMAL(38,20)"),
        ("slider", "DECIMAL(38,20)"),
        ("scale", "DECIMAL(38,20)"),
        ("date", "DATE"),
        ("dateTime", "TIMESTAMPTZ"),
        ("time", "TIME"),
        ("duration", "INTERVAL"),
        (
            "currency",
            "STRUCT(currency VARCHAR, amount DECIMAL(38,20))",
        ),
        (
            "image",
            "STRUCT(name VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)",
        ),
        ("person", "STRUCT(name VARCHAR, email VARCHAR)"),
        ("link", "STRUCT(name VARCHAR, url VARCHAR)"),
        ("hyperlink", "STRUCT(name VARCHAR, url VARCHAR)"),
        (
            "lookup",
            "STRUCT(name VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)",
        ),
        ("canvas", "VARCHAR"),
    ] {
        assert_eq!(logical_type(format_type, false), expected, "{format_type}");
    }
    assert_eq!(logical_type("number", true), "DECIMAL(38,20)[]");
    assert_eq!(logical_type("select", true), "VARCHAR[]");
    assert_eq!(logical_type_alias("canvas"), "JSON");
    assert_eq!(logical_type_alias("number"), "");
}
