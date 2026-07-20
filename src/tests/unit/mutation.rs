use super::*;

#[test]
fn mutation_bodies_match_previous_shape() {
    let column = Box::into_raw(Box::new(SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Column".to_string(),
        format_type: "text".to_string(),
        duckdb_type: "VARCHAR".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: RUST_EXT_COLUMN_EDITABLE,
    }));
    let columns = [
        RustExtWriteColumn {
            handle: column.cast(),
            capabilities: RUST_EXT_COLUMN_EDITABLE,
        },
        RustExtWriteColumn {
            handle: column.cast(),
            capabilities: RUST_EXT_COLUMN_EDITABLE,
        },
    ];
    let values = [
        RustExtInputValue {
            value_type: 5,
            string_value: alloc_string("v"),
            ..Default::default()
        },
        RustExtInputValue {
            value_type: RUST_EXT_INPUT_NULL,
            ..Default::default()
        },
    ];
    assert_eq!(
        insert_body(&columns, &values, 1, 2, RUST_EXT_TABLE_INSERT).unwrap(),
        r#"{"rows":[{"cells":[{"column":"c1","value":"v"}]}]}"#
    );
    assert_eq!(
        update_body(&columns[..1], &values[1..], RUST_EXT_TABLE_UPDATE).unwrap_err(),
        "Superhuman Docs does not support updating a cell to NULL"
    );
    drop(unsafe { Box::from_raw(column) });
    values[0].string_value.free();
}

#[test]
fn mutation_bodies_reduce_rich_duckdb_types_to_api_primitives() {
    let specs = [
        ("currency", false, r#"{"currency":"EUR","amount":10.0}"#),
        (
            "image",
            false,
            r#"{"name":"photo.png","url":"https://example.com/photo.png","height":480,"width":640,"status":"live"}"#,
        ),
        (
            "person",
            false,
            r#"{"name":"Ada Lovelace","email":"ada@example.com"}"#,
        ),
        (
            "hyperlink",
            false,
            r#"{"name":"Example","url":"https://example.com"}"#,
        ),
        (
            "lookup",
            false,
            r#"{"name":"Referenced row","url":"https://coda.io/row","tableId":"tbl-related","tableUrl":"https://coda.io/table","rowId":"row-related"}"#,
        ),
        ("select", true, r#"["One","Two"]"#),
        (
            "currency",
            true,
            r#"[{"currency":"USD","amount":12.34},{"currency":"EUR","amount":56.78}]"#,
        ),
    ];
    let mut column_handles = Vec::new();
    let mut columns = Vec::new();
    let mut values = Vec::new();
    for (index, (format_type, is_array, raw_value)) in specs.iter().enumerate() {
        let column = Box::into_raw(Box::new(SuperhumanDocsColumn {
            id: format!("c{index}"),
            name: format!("Column {index}"),
            format_type: (*format_type).to_string(),
            duckdb_type: logical_type(format_type, *is_array),
            duckdb_type_alias: String::new(),
            is_array: *is_array,
            capabilities: RUST_EXT_COLUMN_EDITABLE,
        }));
        column_handles.push(column);
        columns.push(RustExtWriteColumn {
            handle: column.cast(),
            capabilities: RUST_EXT_COLUMN_EDITABLE,
        });
        values.push(RustExtInputValue {
            value_type: RUST_EXT_INPUT_JSON,
            string_value: alloc_string(raw_value),
            ..Default::default()
        });
    }

    let body: Value = serde_json::from_str(
        &insert_body(&columns, &values, 1, columns.len(), RUST_EXT_TABLE_INSERT).unwrap(),
    )
    .unwrap();
    assert_eq!(
        body,
        json!({
            "rows": [{
                "cells": [
                    {"column": "c0", "value": 10.0},
                    {"column": "c1", "value": "https://example.com/photo.png"},
                    {"column": "c2", "value": "ada@example.com"},
                    {"column": "c3", "value": "https://example.com"},
                    {"column": "c4", "value": "row-related"},
                    {"column": "c5", "value": ["One", "Two"]},
                    {"column": "c6", "value": [12.34, 56.78]},
                ]
            }]
        })
    );

    for value in values {
        value.string_value.free();
    }
    for column in column_handles {
        drop(unsafe { Box::from_raw(column) });
    }
}

#[test]
fn mutation_bodies_reject_incomplete_rich_values() {
    let column = Box::into_raw(Box::new(SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Image".to_string(),
        format_type: "image".to_string(),
        duckdb_type: logical_type("image", false),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: RUST_EXT_COLUMN_EDITABLE,
    }));
    let columns = [RustExtWriteColumn {
        handle: column.cast(),
        capabilities: RUST_EXT_COLUMN_EDITABLE,
    }];
    let value = RustExtInputValue {
        value_type: RUST_EXT_INPUT_JSON,
        string_value: alloc_string(r#"{"name":"missing URL"}"#),
        ..Default::default()
    };
    let error = insert_body(&columns, &[value], 1, 1, RUST_EXT_TABLE_INSERT).unwrap_err();
    assert_eq!(error, "image value is missing its writable field");
    value.string_value.free();
    drop(unsafe { Box::from_raw(column) });
}

#[test]
fn equality_query_uses_json_literal() {
    let value = RustExtInputValue {
        value_type: 5,
        string_value: alloc_string("Ada"),
        ..Default::default()
    };
    let (query, description) = build_equality_query("c1", "Name", value).unwrap();
    assert_eq!(query.as_str(), "c1:\"Ada\"");
    assert_eq!(description.as_str(), "Name = Ada");
    value.string_value.free();
    query.free();
    description.free();
}
