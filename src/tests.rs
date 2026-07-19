use crate::attach::{
    doc_id_from_browser_url, doc_id_from_resolved_link, is_browser_url, read_environment_variable,
    resolve_attach, strip_attach_resource_prefix,
};
use crate::ffi::*;
use crate::json::{column_list_from_json, logical_type, logical_type_alias, rows_from_json};
use crate::model::{
    SuperhumanDocsCell, SuperhumanDocsClientConfig, SuperhumanDocsColumn, SuperhumanDocsRow,
};
use crate::mutation::{build_equality_query, insert_body, update_body};
use crate::scan::scan_value;
use crate::sdk::{validate_token_at, SdkClient};
use crate::secret::{create_secret, free_secret};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::ffi::{c_char, c_void, CStr};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use superhuman_docs::{operations, Client, Error, DEFAULT_BASE_URL};

static NETWORK_UNIT_TEST_LOCK: Mutex<()> = Mutex::new(());

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
            ..Default::default()
        },
        RustExtWriteColumn {
            handle: column.cast(),
            capabilities: RUST_EXT_COLUMN_EDITABLE,
            ..Default::default()
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

#[test]
fn scan_sort_by_returns_owned_string() {
    let column = Box::into_raw(Box::new(SuperhumanDocsColumn {
        id: "createdAt".to_string(),
        name: "createdAt".to_string(),
        format_type: "datetime".to_string(),
        duckdb_type: "TIMESTAMPTZ".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: RUST_EXT_COLUMN_SORT_ASC,
    }));
    let mut sort_by = RustExtString::default();
    assert!(crate::exports::rust_ext_scan_sort_by(
        column.cast(),
        &mut sort_by
    ));
    assert_eq!(sort_by.as_str(), "createdAt");
    assert_ne!(
        sort_by.ptr,
        unsafe { &*column }.id.as_ptr().cast_mut().cast()
    );
    sort_by.free();
    drop(unsafe { Box::from_raw(column) });
}

#[test]
fn scan_unwraps_superhuman_docs_rich_text_fences() {
    let column = SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Name".to_string(),
        format_type: "text".to_string(),
        duckdb_type: "VARCHAR".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: 0,
    };
    let row = SuperhumanDocsRow {
        id: "r1".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
        deleted: false,
        cells: vec![SuperhumanDocsCell {
            column_id: "c1".to_string(),
            value: Value::String("```Alpha```".to_string()),
        }],
    };
    let value = scan_value(&column, &row);
    assert_eq!(value.value.as_str(), "Alpha");
    crate::exports::rust_ext_free_scan_value(value);
}

#[test]
fn scan_normalizes_superhuman_docs_rich_percentage() {
    let column = SuperhumanDocsColumn {
        id: "c1".to_string(),
        name: "Percent".to_string(),
        format_type: "percent".to_string(),
        duckdb_type: "DECIMAL(38,20)".to_string(),
        duckdb_type_alias: String::new(),
        is_array: false,
        capabilities: 0,
    };
    for (raw, expected) in [
        (json!(0.125), "0.125"),
        (json!("25\u{a0}%"), "0.25"),
        (json!("80\u{202f}%"), "0.8"),
    ] {
        let row = SuperhumanDocsRow {
            id: "r1".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            deleted: false,
            cells: vec![SuperhumanDocsCell {
                column_id: "c1".to_string(),
                value: raw,
            }],
        };
        let value = scan_value(&column, &row);
        assert_eq!(value.value.as_str(), expected);
        crate::exports::rust_ext_free_scan_value(value);
    }
}

#[test]
fn token_validation_uses_whoami_status() {
    let _network_guard = NETWORK_UNIT_TEST_LOCK.lock().unwrap();
    let server = MockSuperhumanDocsServer::start();
    validate_token_at(&server.base_url(), "mock-token").unwrap();
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/whoami");
    assert!(
        requests[0]
            .headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Authorization: Bearer mock-token")),
        "expected bearer token in request headers: {}",
        requests[0].headers
    );
    drop(server);

    let server = MockSuperhumanDocsServer::start_with_whoami_status("401 Unauthorized");
    let error = validate_token_at(&server.base_url(), "bad-token").unwrap_err();
    assert_eq!(
        error,
        "Whoami returned HTTP 401, expected 200: not valid JSON"
    );
}

#[test]
fn token_environment_variable_is_read_eagerly() {
    let name = format!(
        "DUCKDB_SUPERHUMAN_DOCS_TOKEN_ENV_TEST_{}",
        std::process::id()
    );
    env::set_var(&name, "resolved-token");
    assert_eq!(read_environment_variable(&name).unwrap(), "resolved-token");
    env::remove_var(&name);
    assert!(read_environment_variable(&name).is_err());
}

#[test]
fn browser_urls_expose_embedded_doc_ids() {
    assert!(is_browser_url(
        "https://coda.io/d/Launch-Status_dAbCDeFGH/Page_su123"
    ));
    assert!(is_browser_url("http://localhost:8080/d/Test_dmock-doc"));
    assert!(!is_browser_url("AbCDeFGH"));
    assert_eq!(
        doc_id_from_browser_url("https://coda.io/d/Launch-Status_dAbCDeFGH/Page_su123"),
        Some("AbCDeFGH".to_string())
    );
    assert_eq!(
        doc_id_from_browser_url("https://example.com/published/launch-status"),
        None
    );
}

#[test]
fn attach_resource_prefixes_are_stripped() {
    for prefix in [
        "coda:",
        "superhuman:",
        "superhuman-docs:",
        "superhuman_docs:",
    ] {
        assert_eq!(
            strip_attach_resource_prefix(&format!("{prefix}https://coda.io/d/_dDoc")),
            "https://coda.io/d/_dDoc"
        );
    }
    assert_eq!(strip_attach_resource_prefix("doc-id"), "doc-id");
}

#[test]
fn resolved_links_map_doc_resources_to_their_containing_doc() {
    assert_eq!(
        doc_id_from_resolved_link(
            r#"{"resource":{"type":"doc","id":"doc-1","href":"https://coda.io/apis/v1/docs/doc-1"}}"#
        )
        .unwrap(),
        "doc-1"
    );
    assert_eq!(
        doc_id_from_resolved_link(
            r#"{"resource":{"type":"table","id":"table-1","href":"https://coda.io/apis/v1/docs/doc-2/tables/table-1"}}"#
        )
        .unwrap(),
        "doc-2"
    );
    let error = doc_id_from_resolved_link(
        r#"{"resource":{"type":"folder","id":"folder-1","href":"https://coda.io/apis/v1/folders/folder-1"}}"#,
    )
    .unwrap_err();
    assert!(error.contains("not contained by a document"));
    assert!(doc_id_from_resolved_link("not-json")
        .unwrap_err()
        .contains("invalid ResolveBrowserLink response"));
}

#[derive(Default)]
struct TestAttachHostContext {
    options: HashMap<String, String>,
    secrets: HashMap<String, String>,
}

unsafe extern "C" fn test_attach_get_option(
    userdata: *mut c_void,
    name: *const c_char,
    out: *mut RustExtString,
    _err: *mut RustExtError,
) -> bool {
    let context = unsafe { &*(userdata.cast::<TestAttachHostContext>()) };
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let value = context
        .options
        .get(name.as_ref())
        .map(String::as_str)
        .unwrap_or("");
    unsafe { out.write(alloc_string(value)) };
    true
}

unsafe extern "C" fn test_attach_lookup_secret(
    userdata: *mut c_void,
    scope: RustExtString,
    _secret_type: *const c_char,
    _secret_key: *const c_char,
    out: *mut RustExtString,
    _err: *mut RustExtError,
) -> bool {
    let context = unsafe { &*(userdata.cast::<TestAttachHostContext>()) };
    let value = context
        .secrets
        .get(scope.as_str())
        .map(String::as_str)
        .unwrap_or("");
    unsafe { out.write(alloc_string(value)) };
    true
}

fn test_attach_host() -> RustExtAttachHost {
    RustExtAttachHost {
        get_option: test_attach_get_option,
        lookup_secret: test_attach_lookup_secret,
    }
}

fn inspect_attach_config<T>(config: RustExtAttachConfig, inspect: T)
where
    T: FnOnce(&SuperhumanDocsClientConfig),
{
    let inner = unsafe { &*config.handle.cast::<SuperhumanDocsClientConfig>() };
    inspect(inner);
    crate::exports::rust_ext_free_attach_config(config);
}

#[test]
fn attach_mutation_options_have_defaults_and_parse_explicit_values() {
    let host = test_attach_host();
    let mut defaults = TestAttachHostContext::default();
    defaults
        .options
        .insert("token".to_string(), "explicit-token".to_string());
    let config = resolve_attach(
        borrow_string("mock-doc"),
        &host,
        (&mut defaults as *mut TestAttachHostContext).cast(),
    )
    .unwrap();
    inspect_attach_config(config, |config| {
        assert!(!config.wait_for_mutations);
        assert_eq!(config.mutation_timeout_seconds, 60);
        assert!(!config.allow_mutation_warnings);
    });

    let mut explicit = TestAttachHostContext::default();
    explicit
        .options
        .insert("token".to_string(), "explicit-token".to_string());
    explicit
        .options
        .insert("wait_for_mutations".to_string(), "true".to_string());
    explicit
        .options
        .insert("mutation_timeout_seconds".to_string(), "17".to_string());
    explicit
        .options
        .insert("allow_mutation_warnings".to_string(), "true".to_string());
    let config = resolve_attach(
        borrow_string("mock-doc"),
        &host,
        (&mut explicit as *mut TestAttachHostContext).cast(),
    )
    .unwrap();
    inspect_attach_config(config, |config| {
        assert!(config.wait_for_mutations);
        assert_eq!(config.mutation_timeout_seconds, 17);
        assert!(config.allow_mutation_warnings);
    });
}

#[test]
fn browser_url_resolution_supports_scoped_general_and_explicit_credentials() {
    let _network_guard = NETWORK_UNIT_TEST_LOCK.lock().unwrap();
    let host = test_attach_host();

    let scoped_server = MockSuperhumanDocsServer::start();
    let mut scoped = TestAttachHostContext::default();
    scoped
        .options
        .insert("api_base".to_string(), scoped_server.base_url());
    scoped.secrets.insert(
        "superhuman_docs:mock-doc".to_string(),
        "scoped-token".to_string(),
    );
    let config = resolve_attach(
        borrow_string("https://coda.io/d/Mock_dmock-doc/Page_su123"),
        &host,
        (&mut scoped as *mut TestAttachHostContext).cast(),
    )
    .unwrap();
    inspect_attach_config(config, |config| {
        assert_eq!(config.resource, "mock-doc");
        assert_eq!(config.credential, "scoped-token");
    });
    assert!(scoped_server.requests().iter().all(|request| request
        .headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Authorization: Bearer scoped-token"))));
    drop(scoped_server);

    let general_server = MockSuperhumanDocsServer::start();
    let mut general = TestAttachHostContext::default();
    general
        .options
        .insert("api_base".to_string(), general_server.base_url());
    general
        .secrets
        .insert("superhuman_docs:".to_string(), "general-token".to_string());
    general.secrets.insert(
        "superhuman_docs:mock-doc".to_string(),
        "canonical-token".to_string(),
    );
    let config = resolve_attach(
        borrow_string("https://example.com/published/launch-status"),
        &host,
        (&mut general as *mut TestAttachHostContext).cast(),
    )
    .unwrap();
    inspect_attach_config(config, |config| {
        assert_eq!(config.resource, "mock-doc");
        assert_eq!(config.credential, "canonical-token");
    });
    assert!(general_server.requests().iter().all(|request| request
        .headers
        .lines()
        .any(|line| line.eq_ignore_ascii_case("Authorization: Bearer general-token"))));
    drop(general_server);

    let explicit_server = MockSuperhumanDocsServer::start();
    let mut explicit = TestAttachHostContext::default();
    explicit
        .options
        .insert("api_base".to_string(), explicit_server.base_url());
    explicit
        .options
        .insert("token".to_string(), "explicit-token".to_string());
    explicit.secrets.insert(
        "superhuman_docs:mock-doc".to_string(),
        "ignored-token".to_string(),
    );
    let config = resolve_attach(
        borrow_string("https://coda.io/d/Mock_dmock-doc"),
        &host,
        (&mut explicit as *mut TestAttachHostContext).cast(),
    )
    .unwrap();
    inspect_attach_config(config, |config| {
        assert_eq!(config.resource, "mock-doc");
        assert_eq!(config.credential, "explicit-token");
    });
}

#[test]
fn noncanonical_browser_url_without_bootstrap_credential_is_targeted_error() {
    let host = test_attach_host();
    let mut context = TestAttachHostContext::default();
    let error = match resolve_attach(
        borrow_string("https://example.com/published/launch-status"),
        &host,
        (&mut context as *mut TestAttachHostContext).cast(),
    ) {
        Ok(config) => {
            crate::exports::rust_ext_free_attach_config(config);
            panic!("browser URL without a bootstrap credential unexpectedly resolved")
        }
        Err(error) => error,
    };
    assert!(error.contains("browser URL attachment requires TOKEN"));
    assert!(error.contains("general superhuman_docs secret"));
}

#[test]
fn secret_policy_is_implemented_by_rust_callback() {
    let result = create_secret(RustExtSecretCreateInput {
        secret_type: borrow_string("superhuman_docs"),
        provider: borrow_string("config"),
        name: borrow_string("test"),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.scope_count, 1);
    assert_eq!(unsafe { &*result.scope }.as_str(), "superhuman_docs:");
    assert_eq!(result.entry_count, 0);
    assert_eq!(result.redact_key_count, 1);
    assert_eq!(unsafe { &*result.redact_keys }.as_str(), "token");
    free_secret(result);

    let option = RustExtNamedValue {
        name: borrow_string("unsupported"),
        value: RustExtInputValue {
            value_type: 5,
            string_value: borrow_string("value"),
            ..Default::default()
        },
    };
    let error = match create_secret(RustExtSecretCreateInput {
        secret_type: borrow_string("superhuman_docs"),
        provider: borrow_string("config"),
        name: borrow_string("test"),
        options: &option,
        option_count: 1,
        ..Default::default()
    }) {
        Ok(result) => {
            free_secret(result);
            panic!("unsupported secret parameter unexpectedly succeeded")
        }
        Err(error) => error,
    };
    assert_eq!(
        error,
        "Unknown named parameter for superhuman_docs secret: unsupported"
    );
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_scan_metadata_and_dml() {
    let server = MockSuperhumanDocsServer::start();
    let table = "superhuman_docs_doc.main.\"Tasks\"";
    let sql = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {}, INCLUDE_ROW_METADATA true);\
         SELECT \"Name\", \"Done\", \"Amount\" FROM {table} ORDER BY \"Name\";\
         SELECT \"Name\" FROM {table} WHERE \"Name\" = 'Alpha';\
         SELECT \"Name\", createdAt FROM {table} ORDER BY createdAt LIMIT 1;\
         INSERT INTO {table} (\"Name\", \"Done\", \"Amount\") VALUES ('Gamma', false, 3.5);\
         UPDATE {table} SET \"Done\" = false, \"Amount\" = 4.5 WHERE \"Name\" = 'Alpha';\
         DELETE FROM {table} WHERE \"Name\" = 'Beta';",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    let output = run_duckdb(&sql);
    assert!(output.contains("Alpha,true,1.25"), "{output}");
    assert!(output.contains("Beta,false,2.5"), "{output}");
    assert!(output.contains("Alpha,2024-01-01"), "{output}");

    let requests = server.requests();
    assert!(
        requests.iter().any(|request| request.method == "GET"
            && request.path == "/docs/mock-doc/tables/tbl1/rows"
            && request.query.contains("valueFormat=rich")),
        "expected rich row values, got {requests:#?}"
    );
    assert!(
        requests.iter().any(|request| request.method == "POST"
            && request.path == "/docs/mock-doc/tables/tbl1/rows"
            && request.body.contains("\"Gamma\"")),
        "expected insert request, got {requests:#?}"
    );
    assert!(
        requests.iter().any(|request| request.method == "GET"
            && request.path == "/docs/mock-doc/tables/tbl1/rows"
            && request.query.contains("query=c_name%3A%22Alpha%22")),
        "expected equality filter pushdown for Alpha, got {requests:#?}"
    );
    assert!(
        requests.iter().any(|request| request.method == "PUT"
            && request.path == "/docs/mock-doc/tables/tbl1/rows/r1"
            && request
                .body
                .contains("\"value\":\"4.50000000000000000000\"")),
        "expected update request, got {requests:#?}"
    );
    assert!(
        requests.iter().any(|request| request.method == "DELETE"
            && request.path == "/docs/mock-doc/tables/tbl1/rows"
            && request.body.contains("\"r2\"")),
        "expected delete request, got {requests:#?}"
    );
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_waits_for_mutations() {
    let server = MockSuperhumanDocsServer::start();
    let table = "superhuman_docs_doc.main.\"Tasks\"";
    let sql = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {},
              WAIT_FOR_MUTATIONS true, MUTATION_TIMEOUT_SECONDS 2,
              ALLOW_MUTATION_WARNINGS false);
         INSERT INTO {table} (\"Name\", \"Done\", \"Amount\") VALUES ('Wait', false, 3.5);
         UPDATE {table} SET \"Done\" = false WHERE \"Name\" = 'Alpha';
         DELETE FROM {table} WHERE \"Name\" = 'Beta';",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    run_duckdb(&sql);
    let requests = server.requests();
    assert!(requests.iter().any(|request| {
        request.method == "GET" && request.path == "/mutationStatus/wait-request"
    }));
    assert!(requests.iter().any(|request| {
        request.method == "GET" && request.path == "/mutationStatus/update-request"
    }));
    assert!(requests.iter().any(|request| {
        request.method == "GET" && request.path == "/mutationStatus/delete-request"
    }));
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_applies_mutation_warning_policy() {
    let server = MockSuperhumanDocsServer::start();
    let table = "superhuman_docs_doc.main.\"Tasks\"";
    let setup = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {},
              WAIT_FOR_MUTATIONS true, MUTATION_TIMEOUT_SECONDS 2);",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    let (success, output) = run_duckdb_command_after_setup(
        &setup,
        &format!(
            "INSERT INTO {table} (\"Name\", \"Done\", \"Amount\") VALUES ('Warn', false, 3.5);"
        ),
    );
    assert!(!success, "{output}");
    assert!(output.contains("completed with a warning"), "{output}");
    assert!(output.contains("cannot be rolled back"), "{output}");

    let server = MockSuperhumanDocsServer::start();
    let sql = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {},
              WAIT_FOR_MUTATIONS true, MUTATION_TIMEOUT_SECONDS 2,
              ALLOW_MUTATION_WARNINGS true);
         INSERT INTO {table} (\"Name\", \"Done\", \"Amount\") VALUES ('Warn', false, 3.5);",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    run_duckdb(&sql);
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_reports_mutation_timeout() {
    let server = MockSuperhumanDocsServer::start();
    let table = "superhuman_docs_doc.main.\"Tasks\"";
    let setup = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {},
              WAIT_FOR_MUTATIONS true, MUTATION_TIMEOUT_SECONDS 1);",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    let (success, output) = run_duckdb_command_after_setup(
        &setup,
        &format!(
            "INSERT INTO {table} (\"Name\", \"Done\", \"Amount\") VALUES ('Timeout', false, 3.5);"
        ),
    );
    assert!(!success, "{output}");
    assert!(
        output.contains("did not complete within 1 seconds"),
        "{output}"
    );
    assert!(output.contains("may occur later"), "{output}");
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_attaches_browser_urls() {
    for prefix in ["coda:", "superhuman:", "superhuman-docs:"] {
        let server = MockSuperhumanDocsServer::start();
        let browser_url = format!("{prefix}https://coda.io/d/Mock_dmock-doc/Table_table-link");
        let sql = format!(
            "LOAD {};
             ATTACH {} AS superhuman_docs_doc
                 (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {});
             SELECT count(*) FROM superhuman_docs_doc.main.\"Tasks\";",
            sql_literal(extension_path()),
            sql_literal(&browser_url),
            sql_literal(&server.base_url())
        );
        let output = run_duckdb(&sql);
        assert!(output.lines().any(|line| line == "2"), "{output}");
        let requests = server.requests();
        assert!(requests.iter().any(|request| {
            request.method == "GET"
                && request.path == "/resolveBrowserLink"
                && request.query.contains("degradeGracefully=false")
                && request.query.contains("table-link")
        }));
        assert!(requests
            .iter()
            .any(|request| { request.method == "GET" && request.path == "/docs/mock-doc/tables" }));
    }

    let direct_server = MockSuperhumanDocsServer::start();
    let direct_sql = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {});
         SELECT count(*) FROM superhuman_docs_doc.main.\"Tasks\";",
        sql_literal(extension_path()),
        sql_literal(&direct_server.base_url())
    );
    run_duckdb(&direct_sql);
    assert!(!direct_server
        .requests()
        .iter()
        .any(|request| request.path == "/resolveBrowserLink"));
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_rejects_non_doc_browser_resources() {
    let server = MockSuperhumanDocsServer::start();
    let setup = format!("LOAD {};", sql_literal(extension_path()));
    let sql = format!(
        "ATTACH 'coda:https://coda.io/folders/folder-link' AS superhuman_docs_doc
             (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {});",
        sql_literal(&server.base_url())
    );
    let (success, output) = run_duckdb_command_after_setup(&setup, &sql);
    assert!(!success, "{output}");
    assert!(output.contains("not contained by a document"), "{output}");
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_token_env_for_attach() {
    let server = MockSuperhumanDocsServer::start();
    let env_name = "DUCKDB_SUPERHUMAN_DOCS_MOCK_API_TOKEN";
    let sql = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS superhuman_docs_attach_env
             (TYPE superhuman_docs, TOKEN_ENV {}, API_BASE {});
         SELECT count(*) FROM superhuman_docs_attach_env.main.\"Tasks\";",
        sql_literal(extension_path()),
        sql_literal(env_name),
        sql_literal(&server.base_url())
    );
    let output = run_duckdb_with_env(&sql, env_name, "mock-token");
    assert!(output.lines().any(|line| line == "2"), "{output}");
    assert!(
        server.requests().iter().all(|request| request
            .headers
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Authorization: Bearer mock-token"))),
        "resolved environment token was not used for every request: {:#?}",
        server.requests()
    );
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_wide_types() {
    let server = MockSuperhumanDocsServer::start();
    let table = "superhuman_docs_doc.main.\"Wide Types\"";
    let sql = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {});\
         SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_catalog = 'superhuman_docs_doc' AND table_schema = 'main' AND table_name = 'Wide Types' \
         ORDER BY ordinal_position;\
         SELECT \"Checkbox\", \"Text\", \"Email\", \"Select\", \
                \"Number\", \"Percent\", \"Slider\", \"Progress\", \"Scale\", \
                \"Date\", \"DateTime\", \"Time\", epoch(\"Duration\"), \
                \"Currency\".currency, \"Currency\".amount, \
                \"Image\".name, \"Image\".url, \"Image\".height, \"Image\".width, \"Image\".status, \
                \"Person\".name, \"Person\".email, \
                \"Hyperlink\".name, \"Hyperlink\".url, \
                \"Lookup\".name, \"Lookup\".url, \"Lookup\".tableId, \"Lookup\".tableUrl, \"Lookup\".rowId, \
                CAST(\"Other\" AS VARCHAR), CAST(\"MultiSelect\" AS VARCHAR), \
                list_transform(\"Durations\", value -> epoch(value)), \
                list_transform(\"Currencies\", value -> value.currency), CAST(\"Others\" AS VARCHAR) \
         FROM {table};
         INSERT INTO {table} (\"Number\", \"Percent\", \"Slider\", \"Progress\", \"Scale\", \"Currency\", \"Image\", \"Person\", \"Hyperlink\", \"Lookup\", \"MultiSelect\", \"Currencies\")
         VALUES (
             123.45, 0.6667, 25, 0.4, 4,
             struct_pack(currency := 'EUR', amount := 10.0),
             struct_pack(name := 'photo.png', url := 'https://example.com/photo.png', height := 480, width := 640, status := 'live'),
             struct_pack(name := 'Ada Lovelace', email := 'ada@example.com'),
             struct_pack(name := 'Example', url := 'https://example.com'),
             struct_pack(name := 'Referenced row', url := 'https://coda.io/row', tableId := 'tbl-related', tableUrl := 'https://coda.io/table', rowId := 'row-related'),
             ['One', 'Two'],
             [struct_pack(currency := 'USD', amount := 12.34), struct_pack(currency := 'EUR', amount := 56.78)]
         );",
        sql_literal(extension_path()),
        sql_literal(&server.base_url()),
    );
    let output = run_duckdb(&sql);
    for expected in [
        "Checkbox,BOOLEAN",
        "Text,VARCHAR",
        "Email,VARCHAR",
        "Select,VARCHAR",
        "Number,\"DECIMAL(38,20)\"",
        "Percent,\"DECIMAL(38,20)\"",
        "Slider,\"DECIMAL(38,20)\"",
        "Progress,\"DECIMAL(38,20)\"",
        "Scale,\"DECIMAL(38,20)\"",
        "Date,DATE",
        "DateTime,TIMESTAMP WITH TIME ZONE",
        "Time,TIME",
        "Duration,INTERVAL",
        "Currency,\"STRUCT(currency VARCHAR, amount DECIMAL(38,20))\"",
        "Image,\"STRUCT(\"\"name\"\" VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)\"",
        "Person,\"STRUCT(\"\"name\"\" VARCHAR, email VARCHAR)\"",
        "Hyperlink,\"STRUCT(\"\"name\"\" VARCHAR, url VARCHAR)\"",
        "Lookup,\"STRUCT(\"\"name\"\" VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)\"",
        "Other,JSON",
        "MultiSelect,VARCHAR[]",
        "Durations,INTERVAL[]",
        "Currencies,\"STRUCT(currency VARCHAR, amount DECIMAL(38,20))[]\"",
        "Others,JSON[]",
        "true,Alpha,ada@example.com,Open",
        "123456789012345678.12345678901234567890",
        "2024-01-02,2024-01-02 03:04:05+00,03:04:05,43200.0,USD,12.34000000000000000000",
        "photo.png,https://example.com/photo.png,480.0,640.0,live",
        "Ada Lovelace,ada@example.com,Example,https://example.com",
        "Referenced row,https://coda.io/row,tbl-related,https://coda.io/table,row-related",
        "nested",
        "One",
        "[43200.0, 86400.0]",
        "[USD, EUR]",
    ] {
        assert!(
            output.contains(expected),
            "expected wide-type output to contain '{expected}', got:\n{output}"
        );
    }
    assert!(
        server.requests().iter().any(|request| {
            request.method == "GET"
                && request.path == "/docs/mock-doc/tables/tbl_wide/rows"
                && request.query.contains("valueFormat=rich")
        }),
        "wide table scan did not request rich values"
    );
    let request = server
        .requests()
        .into_iter()
        .find(|request| {
            request.method == "POST" && request.path == "/docs/mock-doc/tables/tbl_wide/rows"
        })
        .expect("wide table insert was not sent");
    assert!(request.query.contains("disableParsing=false"));
    let decimal = |value| serde_json::from_str::<Value>(value).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&request.body).unwrap(),
        json!({
            "rows": [{
                "cells": [
                    {"column": "c_number", "value": decimal("123.45000000000000000000")},
                    {"column": "c_percent", "value": decimal("0.66670000000000000000")},
                    {"column": "c_slider", "value": decimal("25.00000000000000000000")},
                    {"column": "c_progress", "value": decimal("0.40000000000000000000")},
                    {"column": "c_scale", "value": decimal("4.00000000000000000000")},
                    {"column": "c_currency", "value": decimal("10.00000000000000000000")},
                    {"column": "c_image", "value": "https://example.com/photo.png"},
                    {"column": "c_person", "value": "ada@example.com"},
                    {"column": "c_hyperlink", "value": "https://example.com"},
                    {"column": "c_lookup", "value": "row-related"},
                    {"column": "c_multiselect", "value": ["One", "Two"]},
                    {"column": "c_currencies", "value": [
                        decimal("12.34000000000000000000"),
                        decimal("56.78000000000000000000")
                    ]},
                ]
            }]
        })
    );
}

#[test]
#[ignore]
fn duckdb_mock_superhuman_docs_rejects_explicit_transactions() {
    let server = MockSuperhumanDocsServer::start();
    let setup = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN 'mock-token', API_BASE {});",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    let (success, output) = run_duckdb_command_after_setup(
        &setup,
        "BEGIN TRANSACTION; INSERT INTO superhuman_docs_doc.main.\"Tasks\" (\"Name\", \"Done\", \"Amount\") VALUES ('Txn', false, 9.0); ROLLBACK;",
    );
    assert!(
        !success
            && output.contains("Superhuman Docs does not support explicit DuckDB transactions"),
        "{output}\nrequests: {:#?}",
        server.requests()
    );
    assert!(
        !server
            .requests()
            .iter()
            .any(|request| request.method == "POST"),
        "transactional write should be rejected before HTTP mutation"
    );
}

#[test]
#[ignore]
fn real_superhuman_docs_api_smoke() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let page_name = format!("duckdb-superhuman-docs-test-{run_id}");
    let table_name = format!("duckdb_superhuman_docs_test_{run_id}");
    let page = create_test_page(&endpoint, &credential, &resource, &page_name, &table_name)
        .expect("failed to create Superhuman Docs test page");
    let page_id = page
        .get("id")
        .and_then(Value::as_str)
        .expect("create page response did not include id")
        .to_string();
    let cleanup = PageCleanup {
        endpoint: endpoint.clone(),
        credential: credential.clone(),
        resource: resource.clone(),
        page_id: page_id.clone(),
    };

    let discovered_table =
        wait_for_page_table(&endpoint, &credential, &resource, &page_id, &table_name)
            .expect("timed out waiting for generated Superhuman Docs table");
    let table_id = discovered_table
        .get("id")
        .and_then(Value::as_str)
        .expect("generated table did not include id");
    assert_required_columns(&endpoint, &credential, &resource, table_id)
        .expect("generated table is missing expected columns");
    let actual_table_name = discovered_table
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&table_name);

    run_duckdb_success_case(&resource, &credential, &endpoint, actual_table_name);
    run_duckdb_metadata_case(&resource, &credential, &endpoint, actual_table_name);
    drop(cleanup);
}

#[test]
#[ignore]
fn real_superhuman_docs_api_wide_types() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    run_duckdb_real_wide_types_schema_case(
        &resource,
        &credential,
        &endpoint,
        REAL_WIDE_TYPES_TABLE,
    );
    run_duckdb_real_wide_types_dml_case(&resource, &credential, &endpoint);
}

#[test]
#[ignore]
fn real_superhuman_docs_api_wide_types_fixture_select_only() {
    let credential = required_env("SUPERHUMAN_DOCS_TEST_API_TOKEN");
    let endpoint = env::var("SUPERHUMAN_DOCS_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("SUPERHUMAN_DOCS_TEST_DOC_ID");

    run_duckdb_real_wide_types_schema_case(
        &resource,
        &credential,
        &endpoint,
        REAL_WIDE_TYPES_FIXTURE_TABLE,
    );
    run_duckdb_real_wide_types_fixture_select_case(&resource, &credential, &endpoint);
}

#[derive(Clone, Debug)]
struct MockRequest {
    method: String,
    path: String,
    query: String,
    headers: String,
    body: String,
}

struct MockSuperhumanDocsServer {
    address: String,
    requests: Arc<Mutex<Vec<MockRequest>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockSuperhumanDocsServer {
    fn start() -> Self {
        Self::start_with_whoami_status("200 OK")
    }

    fn start_with_whoami_status(whoami_status: &'static str) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind mock Superhuman Docs server");
        let address = listener
            .local_addr()
            .expect("failed to read mock Superhuman Docs server address")
            .to_string();
        listener
            .set_nonblocking(true)
            .expect("failed to configure mock Superhuman Docs server");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_requests = Arc::clone(&requests);
        let thread_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || loop {
            if thread_shutdown.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => handle_mock_connection(stream, &thread_requests, whoami_status),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => {
                    panic!("mock Superhuman Docs server failed to accept connection: {err}")
                }
            }
        });
        Self {
            address,
            requests,
            shutdown,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    fn requests(&self) -> Vec<MockRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for MockSuperhumanDocsServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .expect("mock Superhuman Docs server thread panicked");
        }
    }
}

fn handle_mock_connection(
    mut stream: TcpStream,
    requests: &Arc<Mutex<Vec<MockRequest>>>,
    whoami_status: &'static str,
) {
    let mut buffer = Vec::new();
    let mut temp = [0; 1024];
    let header_end;
    loop {
        let read = stream
            .read(&mut temp)
            .expect("failed to read mock Superhuman Docs request");
        if read == 0 {
            return;
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(position) = find_header_end(&buffer) {
            header_end = position;
            break;
        }
    }

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream
            .read(&mut temp)
            .expect("failed to read mock Superhuman Docs request body");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
    }

    let request_line = headers.lines().next().unwrap_or("");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("").to_string();
    let target = request_parts.next().unwrap_or("");
    let (path, query) = target
        .split_once('?')
        .map(|(path, query)| (path.to_string(), query.to_string()))
        .unwrap_or_else(|| (target.to_string(), String::new()));
    let body =
        String::from_utf8_lossy(&buffer[body_start..body_start + content_length]).to_string();
    let request_occurrence = {
        let mut requests = requests.lock().unwrap();
        requests.push(MockRequest {
            method: method.clone(),
            path: path.clone(),
            query: query.clone(),
            headers,
            body: body.clone(),
        });
        requests
            .iter()
            .filter(|request| request.method == method && request.path == path)
            .count()
    };

    let (status, response_body) = mock_response(
        &method,
        &path,
        &query,
        &body,
        request_occurrence,
        whoami_status,
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    stream
        .write_all(response.as_bytes())
        .expect("failed to write mock Superhuman Docs response");
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn mock_response(
    method: &str,
    path: &str,
    query: &str,
    body: &str,
    request_occurrence: usize,
    whoami_status: &'static str,
) -> (&'static str, String) {
    match (method, path) {
        ("GET", "/whoami") => (whoami_status, "not valid JSON".to_string()),
        ("GET", "/resolveBrowserLink") if query.contains("folder-link") => (
            "200 OK",
            json!({
                "type": "apiLink",
                "href": "https://coda.io/apis/v1/resolveBrowserLink",
                "resource": {
                    "type": "folder",
                    "id": "folder-1",
                    "href": "https://coda.io/apis/v1/folders/folder-1"
                }
            })
            .to_string(),
        ),
        ("GET", "/resolveBrowserLink") if query.contains("table-link") => (
            "200 OK",
            json!({
                "type": "apiLink",
                "href": "https://coda.io/apis/v1/resolveBrowserLink",
                "resource": {
                    "type": "table",
                    "id": "tbl1",
                    "href": "https://coda.io/apis/v1/docs/mock-doc/tables/tbl1"
                }
            })
            .to_string(),
        ),
        ("GET", "/resolveBrowserLink") => (
            "200 OK",
            json!({
                "type": "apiLink",
                "href": "https://coda.io/apis/v1/resolveBrowserLink",
                "resource": {
                    "type": "doc",
                    "id": "mock-doc",
                    "href": "https://coda.io/apis/v1/docs/mock-doc"
                }
            })
            .to_string(),
        ),
        ("GET", "/docs/mock-doc/tables") => (
            "200 OK",
            json!({
                "items": [
                    {"id": "tbl1", "name": "Tasks", "tableType": "table"},
                    {"id": "tbl_wide", "name": "Wide Types", "tableType": "table"}
                ]
            })
            .to_string(),
        ),
        ("GET", "/docs/mock-doc/tables/tbl1/columns") => (
            "200 OK",
            json!({
                "items": [
                    {"id": "c_name", "name": "Name", "calculated": false, "format": {"type": "text", "isArray": false}},
                    {"id": "c_done", "name": "Done", "calculated": false, "format": {"type": "checkbox", "isArray": false}},
                    {"id": "c_amount", "name": "Amount", "calculated": false, "format": {"type": "number", "isArray": false}}
                ]
            })
            .to_string(),
        ),
        ("GET", "/docs/mock-doc/tables/tbl_wide/columns") => (
            "200 OK",
            json!({
                "items": [
                    {"id": "c_checkbox", "name": "Checkbox", "calculated": false, "format": {"type": "checkbox", "isArray": false}},
                    {"id": "c_text", "name": "Text", "calculated": false, "format": {"type": "text", "isArray": false}},
                    {"id": "c_email", "name": "Email", "calculated": false, "format": {"type": "email", "isArray": false}},
                    {"id": "c_select", "name": "Select", "calculated": false, "format": {"type": "select", "isArray": false}},
                    {"id": "c_number", "name": "Number", "calculated": false, "format": {"type": "number", "isArray": false}},
                    {"id": "c_percent", "name": "Percent", "calculated": false, "format": {"type": "percent", "isArray": false}},
                    {"id": "c_slider", "name": "Slider", "calculated": false, "format": {"type": "slider", "isArray": false}},
                    {"id": "c_progress", "name": "Progress", "calculated": false, "format": {"type": "slider", "isArray": false, "displayType": "progress"}},
                    {"id": "c_scale", "name": "Scale", "calculated": false, "format": {"type": "scale", "isArray": false}},
                    {"id": "c_date", "name": "Date", "calculated": false, "format": {"type": "date", "isArray": false}},
                    {"id": "c_datetime", "name": "DateTime", "calculated": false, "format": {"type": "dateTime", "isArray": false}},
                    {"id": "c_time", "name": "Time", "calculated": false, "format": {"type": "time", "isArray": false}},
                    {"id": "c_duration", "name": "Duration", "calculated": false, "format": {"type": "duration", "isArray": false}},
                    {"id": "c_currency", "name": "Currency", "calculated": false, "format": {"type": "currency", "isArray": false}},
                    {"id": "c_image", "name": "Image", "calculated": false, "format": {"type": "image", "isArray": false}},
                    {"id": "c_person", "name": "Person", "calculated": false, "format": {"type": "person", "isArray": false}},
                    {"id": "c_hyperlink", "name": "Hyperlink", "calculated": false, "format": {"type": "hyperlink", "isArray": false}},
                    {"id": "c_lookup", "name": "Lookup", "calculated": false, "format": {"type": "lookup", "isArray": false}},
                    {"id": "c_other", "name": "Other", "calculated": false, "format": {"type": "canvas", "isArray": false}},
                    {"id": "c_multiselect", "name": "MultiSelect", "calculated": false, "format": {"type": "select", "isArray": true}},
                    {"id": "c_durations", "name": "Durations", "calculated": false, "format": {"type": "duration", "isArray": true}},
                    {"id": "c_currencies", "name": "Currencies", "calculated": false, "format": {"type": "currency", "isArray": true}},
                    {"id": "c_others", "name": "Others", "calculated": false, "format": {"type": "canvas", "isArray": true}}
                ]
            })
            .to_string(),
        ),
        ("GET", "/docs/mock-doc/tables/tbl1/rows") => ("200 OK", mock_rows_response(query)),
        ("GET", "/docs/mock-doc/tables/tbl_wide/rows") => {
            ("200 OK", mock_wide_rows_response(query))
        }
        ("POST", "/docs/mock-doc/tables/tbl1/rows") => {
            let request_id = if body.contains("\"Warn\"") {
                "warning-request"
            } else if body.contains("\"Timeout\"") {
                "timeout-request"
            } else if body.contains("\"Wait\"") {
                "wait-request"
            } else {
                "insert-request"
            };
            (
                "202 Accepted",
                json!({"requestId": request_id, "addedRowIds": ["new-row"]}).to_string(),
            )
        }
        ("POST", "/docs/mock-doc/tables/tbl_wide/rows") => (
            "202 Accepted",
            json!({"requestId": "wide-insert-request", "addedRowIds": ["wide-new-row"]})
                .to_string(),
        ),
        ("PUT", "/docs/mock-doc/tables/tbl1/rows/r1") => (
            "202 Accepted",
            json!({"requestId": "update-request", "id": "r1"}).to_string(),
        ),
        ("DELETE", "/docs/mock-doc/tables/tbl1/rows") => (
            "202 Accepted",
            json!({"requestId": "delete-request", "rowIds": ["r2"]}).to_string(),
        ),
        ("GET", "/mutationStatus/warning-request") => (
            "200 OK",
            json!({"completed": true, "warning": "mock mutation warning"}).to_string(),
        ),
        ("GET", "/mutationStatus/timeout-request") => {
            ("200 OK", json!({"completed": false}).to_string())
        }
        ("GET", "/mutationStatus/wait-request") if request_occurrence == 1 => (
            "404 Not Found",
            json!({"message": "mutation status is not visible yet"}).to_string(),
        ),
        ("GET", path) if path.starts_with("/mutationStatus/") => {
            ("200 OK", json!({"completed": true}).to_string())
        }
        _ => (
            "404 Not Found",
            json!({"error": format!("unexpected mock request {method} {path}")}).to_string(),
        ),
    }
}

fn mock_wide_rows_response(query: &str) -> String {
    if query.contains("syncToken=") {
        return json!({"items": []}).to_string();
    }
    let precise_number: Value =
        serde_json::from_str("123456789012345678.12345678901234567890").unwrap();
    json!({
        "items": [{
            "id": "wide-row",
            "values": {
                "c_checkbox": true,
                "c_text": "Alpha",
                "c_email": "ada@example.com",
                "c_select": "Open",
                "c_number": precise_number,
                "c_percent": 0.125,
                "c_slider": 42,
                "c_progress": 0.4,
                "c_scale": 5,
                "c_date": "2024-01-02",
                "c_datetime": "2024-01-02T03:04:05Z",
                "c_time": "03:04:05",
                "c_duration": 0.5,
                "c_currency": {
                    "@context": "http://schema.org/", "@type": "MonetaryAmount",
                    "currency": "USD", "amount": "12.34"
                },
                "c_image": {
                    "@context": "http://schema.org/", "@type": "ImageObject",
                    "name": "photo.png", "url": "https://example.com/photo.png",
                    "height": 480, "width": 640, "status": "live"
                },
                "c_person": {
                    "@context": "http://schema.org/", "@type": "Person",
                    "name": "Ada Lovelace", "email": "ada@example.com"
                },
                "c_hyperlink": {
                    "@context": "http://schema.org/", "@type": "WebPage",
                    "name": "Example", "url": "https://example.com"
                },
                "c_lookup": {
                    "@context": "http://schema.org/", "@type": "StructuredValue",
                    "name": "Referenced row", "url": "https://coda.io/row",
                    "tableId": "tbl-related", "tableUrl": "https://coda.io/table", "rowId": "row-related"
                },
                "c_other": {"nested": [1, 2, 3]},
                "c_multiselect": ["One", "Two"],
                "c_durations": [0.5, 1],
                "c_currencies": [
                    {"@type": "MonetaryAmount", "currency": "USD", "amount": "12.34"},
                    {"@type": "MonetaryAmount", "currency": "EUR", "amount": "56.78"}
                ],
                "c_others": [{"nested": 1}, {"nested": 2}]
            }
        }],
        "nextSyncToken": "wide-sync-token"
    })
    .to_string()
}

fn mock_rows_response(query: &str) -> String {
    if query.contains("syncToken=") {
        return json!({"items": []}).to_string();
    }
    let all_rows = vec![
        json!({
            "id": "r1",
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-02T00:00:00Z",
            "values": {
                "c_name": "Alpha",
                "c_done": true,
                "c_amount": 1.25
            }
        }),
        json!({
            "id": "r2",
            "createdAt": "2024-01-03T00:00:00Z",
            "updatedAt": "2024-01-04T00:00:00Z",
            "values": {
                "c_name": "Beta",
                "c_done": false,
                "c_amount": 2.5
            }
        }),
    ];
    let rows: Vec<Value> = if query.contains("query=c_name") && query.contains("Alpha") {
        vec![all_rows[0].clone()]
    } else if query.contains("query=c_name") && query.contains("Beta") {
        vec![all_rows[1].clone()]
    } else if query.contains("sortBy=createdAt") && query.contains("limit=1") {
        vec![all_rows[0].clone()]
    } else {
        all_rows
    };
    json!({"items": rows, "nextSyncToken": "sync-token"}).to_string()
}

struct PageCleanup {
    endpoint: String,
    credential: String,
    resource: String,
    page_id: String,
}

impl Drop for PageCleanup {
    fn drop(&mut self) {
        if let Ok(sdk) = SdkClient::at(&self.endpoint, &self.credential) {
            let _ = sdk.execute(|client| {
                client.docs().pages().delete(operations::DeletePageInput {
                    doc_id: self.resource.clone(),
                    page_id_or_name: self.page_id.clone(),
                })
            });
        }
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set in the environment"))
}

fn api_json<T>(
    sdk: &SdkClient,
    operation: impl FnOnce(&Client) -> Result<T, Error>,
) -> Result<Value, String> {
    json_body(sdk.execute(operation)?)
}

fn json_body(body: String) -> Result<Value, String> {
    if body.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }
}

fn paged_items<T>(
    sdk: &SdkClient,
    mut operation: impl FnMut(&Client, Option<String>) -> Result<T, Error>,
) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut page_token = None;
    loop {
        let root = api_json(sdk, |client| operation(client, page_token.take()))?;
        if let Some(items) = root.get("items").and_then(Value::as_array) {
            out.extend(items.iter().cloned());
        }
        page_token = root
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if page_token.is_none() {
            return Ok(out);
        }
    }
}

fn create_test_page(
    endpoint: &str,
    credential: &str,
    resource: &str,
    page_name: &str,
    table_name: &str,
) -> Result<Value, String> {
    let html = format!(
        "<h1>{table_name}</h1>\
         <table>\
         <caption>{table_name}</caption>\
         <thead><tr><th>Name</th><th>Done</th><th>Amount</th></tr></thead>\
         <tbody>\
         <tr><td>Alpha</td><td>true</td><td>1.25</td></tr>\
         <tr><td>Beta</td><td>false</td><td>2.5</td></tr>\
         </tbody>\
         </table>"
    );
    create_page_with_html(endpoint, credential, resource, page_name, html)
}

fn create_page_with_html(
    endpoint: &str,
    credential: &str,
    resource: &str,
    page_name: &str,
    html: String,
) -> Result<Value, String> {
    let payload = json!({
        "name": page_name,
        "pageContent": {
            "type": "canvas",
            "canvasContent": {
                "format": "html",
                "content": html,
            },
        },
    })
    .to_string();
    let sdk = SdkClient::at(endpoint, credential)?;
    let body = sdk.execute_with_body(payload, |client| {
        client.docs().pages().create(operations::CreatePageInput {
            doc_id: resource.to_string(),
            payload: operations::PageCreate {
                name: Some(page_name.to_string()),
                subtitle: None,
                icon_name: None,
                image_url: None,
                parent_page_id: None,
                page_content: None,
            },
        })
    })?;
    json_body(body)
}

fn wait_for_page_table(
    endpoint: &str,
    credential: &str,
    resource: &str,
    page_id: &str,
    wanted_table_name: &str,
) -> Result<Value, String> {
    let sdk = SdkClient::at(endpoint, credential)?;
    for _ in 0..40 {
        let tables = paged_items(&sdk, |client, page_token| {
            client.tables().list(operations::ListTablesInput {
                doc_id: resource.to_string(),
                limit: Some(100),
                page_token,
                sort_by: None,
                table_types: None,
            })
        })?;
        for table in &tables {
            let parent_id = table
                .get("parent")
                .and_then(|parent| parent.get("id"))
                .and_then(Value::as_str);
            let table_type = table
                .get("tableType")
                .and_then(Value::as_str)
                .unwrap_or("table");
            if parent_id == Some(page_id) && table_type.eq_ignore_ascii_case("table") {
                let table_name = table.get("name").and_then(Value::as_str).unwrap_or("");
                if table_name != wanted_table_name {
                    eprintln!(
                        "Superhuman Docs named integration table '{table_name}'; requested '{wanted_table_name}'"
                    );
                }
                return Ok(table.clone());
            }
        }
        thread::sleep(Duration::from_secs(3));
    }
    Err("timed out waiting for Superhuman Docs table".to_string())
}

fn assert_required_columns(
    endpoint: &str,
    credential: &str,
    resource: &str,
    table_id: &str,
) -> Result<(), String> {
    let sdk = SdkClient::at(endpoint, credential)?;
    let columns = paged_items(&sdk, |client, page_token| {
        client
            .tables()
            .columns()
            .list(operations::ListColumnsInput {
                doc_id: resource.to_string(),
                table_id_or_name: table_id.to_string(),
                limit: Some(100),
                page_token,
                visible_only: Some(false),
            })
    })?;
    for required in ["Name", "Done", "Amount"] {
        let found = columns
            .iter()
            .any(|column| column.get("name").and_then(Value::as_str) == Some(required));
        if !found {
            return Err(format!("missing required column {required}"));
        }
    }
    Ok(())
}

const REAL_WIDE_TYPES_TABLE: &str = "duckdb_coda_wide_types";
const REAL_WIDE_TYPES_FIXTURE_TABLE: &str = "duckdb_coda_wide_types_fixture";
const REAL_WIDE_TYPES_COLUMNS: &[(&str, &str)] = &[
    ("col_text", "VARCHAR"),
    ("col_number", "DECIMAL(38,20)"),
    ("col_percentage", "DECIMAL(38,20)"),
    (
        "col_currency",
        "STRUCT(currency VARCHAR, amount DECIMAL(38,20))",
    ),
    ("col_slider", "DECIMAL(38,20)"),
    ("col_progress", "DECIMAL(38,20)"),
    ("col_scale", "DECIMAL(38,20)"),
    ("col_date", "DATE"),
    ("col_time", "TIME"),
    ("col_datetime", "TIMESTAMP WITH TIME ZONE"),
    ("col_duration", "INTERVAL"),
    ("col_canvas", "JSON"),
    ("col_checkbox", "BOOLEAN"),
    ("col_people", "STRUCT(\"name\" VARCHAR, email VARCHAR)"),
    ("col_link", "STRUCT(\"name\" VARCHAR, url VARCHAR)"),
    ("col_select", "VARCHAR"),
    ("col_reaction", "JSON[]"),
    (
        "col_relation",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)",
    ),
    (
        "col_reference",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, tableId VARCHAR, tableUrl VARCHAR, rowId VARCHAR)",
    ),
    ("col_button", "JSON"),
    (
        "col_image",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)[]",
    ),
    (
        "col_image_url",
        "STRUCT(\"name\" VARCHAR, url VARCHAR, height DOUBLE, width DOUBLE, status VARCHAR)",
    ),
    ("col_file", "JSON[]"),
    (
        "col_virtual_createdby",
        "STRUCT(\"name\" VARCHAR, email VARCHAR)",
    ),
    ("col_toggle", "BOOLEAN"),
    ("col_email", "VARCHAR"),
];

fn superhuman_docs_attach_sql(resource: &str, credential: &str, endpoint: &str) -> String {
    format!(
        "LOAD {}; ATTACH {} AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN {}, API_BASE {}, WAIT_FOR_MUTATIONS true);",
        sql_literal(extension_path()),
        sql_literal(resource),
        sql_literal(credential),
        sql_literal(endpoint),
    )
}

fn run_duckdb_real_wide_types_schema_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
    table_name: &str,
) {
    let expected = REAL_WIDE_TYPES_COLUMNS
        .iter()
        .map(|(name, data_type)| {
            // The fixture enables multiple selections for its reference column.
            let data_type =
                if table_name == REAL_WIDE_TYPES_FIXTURE_TABLE && *name == "col_reference" {
                    format!("{data_type}[]")
                } else {
                    data_type.to_string()
                };
            format!("({}, {})", sql_literal(name), sql_literal(&data_type))
        })
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"{}
        WITH expected(column_name, data_type) AS (VALUES {expected}),
             actual AS (
                 SELECT column_name, data_type FROM information_schema.columns
                 WHERE table_catalog = 'superhuman_docs_doc' AND table_schema = 'main' AND table_name = {}
             )
        SELECT count(*) = {} AND (SELECT count(*) FROM actual) = {}
               AND bool_and(actual.data_type = expected.data_type) AS schema_ok
        FROM expected JOIN actual USING (column_name);"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(table_name),
        REAL_WIDE_TYPES_COLUMNS.len(),
        REAL_WIDE_TYPES_COLUMNS.len(),
    );
    let output = run_duckdb(&sql);
    assert!(
        output.contains("schema_ok\ntrue"),
        "expected {table_name} to match the captured wide-type schema, got:\n{output}"
    );
}

struct WideRowCleanup {
    resource: String,
    credential: String,
    endpoint: String,
    row_name: String,
    active: bool,
}

impl WideRowCleanup {
    fn sql(&self) -> String {
        format!(
            "{} DELETE FROM superhuman_docs_doc.main.{} WHERE col_text = {};",
            superhuman_docs_attach_sql(&self.resource, &self.credential, &self.endpoint),
            sql_ident(REAL_WIDE_TYPES_TABLE),
            sql_literal(&self.row_name),
        )
    }

    fn delete(&mut self) {
        run_duckdb(&self.sql());
        self.active = false;
    }
}

impl Drop for WideRowCleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = Command::new("build/release/duckdb")
            .env("TZ", "UTC")
            .args(["-batch", "-csv", ":memory:", "-c", &self.sql()])
            .output();
    }
}

fn run_duckdb_real_wide_types_dml_case(resource: &str, credential: &str, endpoint: &str) {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let row_name = format!("duckdb-superhuman-docs-wide-{run_id}");
    let table = format!(
        "superhuman_docs_doc.main.{}",
        sql_ident(REAL_WIDE_TYPES_TABLE)
    );
    let mut cleanup = WideRowCleanup {
        resource: resource.to_string(),
        credential: credential.to_string(),
        endpoint: endpoint.to_string(),
        row_name: row_name.clone(),
        active: true,
    };
    let insert_sql = format!(
        "{} INSERT INTO {table} (\
             col_text, col_number, col_percentage, col_slider, col_progress, col_scale,\
             col_date, col_time, col_datetime, col_duration, col_canvas, col_checkbox, col_select,\
             col_toggle, col_email\
         ) VALUES (\
             {}, 123.5, 0.375, 11, 0.4, 4, DATE '2026-07-17', TIME '12:34:56',\
             TIMESTAMPTZ '2026-07-17 12:34:56+00', INTERVAL '1 hour 2 minutes 3 seconds', 'wide canvas',\
             true, 'Done', false, 'probe@example.com'\
         );",
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(&row_name),
    );
    run_duckdb(&insert_sql);

    let select_sql = format!(
        r#"{} SELECT col_text, col_number, col_percentage, col_slider, col_progress, col_scale,
             col_date, col_time, col_datetime, col_duration,
             col_canvas::VARCHAR LIKE '%wide canvas%' AS canvas_ok, col_checkbox, col_select,
             col_toggle, col_email
         FROM {table} WHERE col_text = {};"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
        sql_literal(&row_name),
    );
    let mut output = String::new();
    for _ in 0..20 {
        output = run_duckdb(&select_sql);
        if output.contains(&row_name) {
            break;
        }
        thread::sleep(Duration::from_secs(3));
    }
    cleanup.delete();

    for expected in [
        row_name.as_str(),
        "123.50000000000000000000,0.37500000000000000000,11.00000000000000000000,0.40000000000000000000,4.00000000000000000000",
        "2026-07-17,12:34:56,2026-07-17 12:34:56+00,01:02:03,true,true,Done,false,probe@example.com",
    ] {
        assert!(
            output.contains(expected),
            "expected mutable wide-type row output to contain '{expected}', got:\n{output}"
        );
    }
}

fn run_duckdb_real_wide_types_fixture_select_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
) {
    let table = format!(
        "superhuman_docs_doc.main.{}",
        sql_ident(REAL_WIDE_TYPES_FIXTURE_TABLE)
    );
    let sql = format!(
        r#"{}
         SELECT count(*) = 10 AS row_count_ok FROM {table};
         SELECT col_text, col_number, col_slider, col_progress, col_scale
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;
         SELECT col_text, col_percentage, col_currency.currency, col_currency.amount, col_date,
                col_time, col_datetime, col_duration, col_checkbox, col_people.name AS person_name,
                col_link.url AS link_url, col_select, col_relation.rowId AS relation_row_id,
                col_image_url.url AS image_url, col_virtual_createdby.email AS created_by,
                col_toggle, col_email
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;
         SELECT col_text, col_canvas::VARCHAR AS canvas_json,
                list_count(col_reaction) AS reaction_count,
                list_count(col_reference) AS reference_count,
                col_button::VARCHAR AS button_json,
                col_image IS NULL AS image_null, col_file IS NULL AS file_null
         FROM {table} WHERE col_text IN ('Alice Johnson', 'James O''Brien') ORDER BY col_text;"#,
        superhuman_docs_attach_sql(resource, credential, endpoint),
    );
    let output = run_duckdb(&sql);
    for expected in [
        "row_count_ok\ntrue",
        "Alice Johnson,42.00000000000000000000,3.00000000000000000000,0.10000000000000000000,3.00000000000000000000",
        "\"James O'Brien\",12.50000000000000000000,10.00000000000000000000,1.00000000000000000000,2.00000000000000000000",
        "Alice Johnson,0.25000000000000000000,EUR,1200.50000000000000000000,2024-01-15,09:00:00,2024-01-15 08:00:00+00,00:01:00,true,NULL,https://example.com/project-alpha,Not started,i-UcMngti-L2,https://picsum.photos/seed/row1/200/150,felix.testuser@outlook.com,false,alice@example.com",
        "\"James O'Brien\",0.55000000000000000000,EUR,44.99000000000000000000,2025-12-25,23:00:00,2025-12-25 22:00:00+00,00:00:07,false,Felix Testuser,https://example.com/final-report,Done,NULL,https://picsum.photos/seed/row10/200/150,felix.testuser@outlook.com,false,james@example.com",
        "Alice Johnson,\"\"\"**Project kickoff** meeting notes. Action items TBD.\"\"\",1,0,\"\"\"\"\"\",true,true",
        "\"James O'Brien\",\"\"\"Final report submitted. *Project closed.*\"\"\",1,0,\"\"\"\"\"\",true,true",
    ] {
        assert!(
            output.contains(expected),
            "expected immutable wide-type fixture output to contain '{expected}', got:\n{output}"
        );
    }
}

fn run_duckdb_success_case(resource: &str, credential: &str, endpoint: &str, table_name: &str) {
    let table = format!("superhuman_docs_doc.main.{}", sql_ident(table_name));
    let sql = format!(
        "LOAD {};\
         ATTACH {} AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN {}, API_BASE {}, WAIT_FOR_MUTATIONS true);\
         SELECT {}, {}, {} FROM {table} ORDER BY {};\
         SELECT * FROM {table} WHERE {} != '';\
         INSERT INTO {table} ({}, {}, {}) VALUES ('Gamma', false, 3.5);\
         UPDATE {table} SET {} = false, {} = 4.5 WHERE {} = 'Alpha';\
         DELETE FROM {table} WHERE {} = 'Beta';",
        sql_literal(extension_path()),
        sql_literal(resource),
        sql_literal(credential),
        sql_literal(endpoint),
        sql_ident("Name"),
        sql_ident("Done"),
        sql_ident("Amount"),
        sql_ident("Name"),
        sql_ident("Name"),
        sql_ident("Name"),
        sql_ident("Done"),
        sql_ident("Amount"),
        sql_ident("Done"),
        sql_ident("Amount"),
        sql_ident("Name"),
        sql_ident("Name"),
    );
    let output = run_duckdb(&sql);
    assert!(
        output.contains("Alpha") && output.contains("Beta"),
        "expected initial SELECT output to include seed rows, got:\n{output}"
    );
}

fn run_duckdb_metadata_case(resource: &str, credential: &str, endpoint: &str, table_name: &str) {
    let table = format!("superhuman_docs_doc.main.{}", sql_ident(table_name));
    let sql = format!(
        "LOAD {};\
         ATTACH {} AS superhuman_docs_doc (TYPE superhuman_docs, TOKEN {}, API_BASE {}, INCLUDE_ROW_METADATA true);\
         SELECT column_name, data_type \
         FROM information_schema.columns \
         WHERE table_catalog = 'superhuman_docs_doc' \
           AND table_schema = 'main' \
           AND table_name = {} \
           AND column_name IN ('createdAt', 'updatedAt') \
         ORDER BY column_name;\
         SELECT {}, typeof(createdAt), typeof(updatedAt), createdAt IS NOT NULL, updatedAt IS NOT NULL \
         FROM {table} \
         WHERE {} = 'Alpha';",
        sql_literal(extension_path()),
        sql_literal(resource),
        sql_literal(credential),
        sql_literal(endpoint),
        sql_literal(table_name),
        sql_ident("Name"),
        sql_ident("Name"),
    );
    let output = run_duckdb(&sql);
    for expected in [
        "createdAt,TIMESTAMP WITH TIME ZONE",
        "updatedAt,TIMESTAMP WITH TIME ZONE",
        "Alpha,TIMESTAMP WITH TIME ZONE,TIMESTAMP WITH TIME ZONE,true,true",
    ] {
        assert!(
            output.contains(expected),
            "expected metadata output line '{expected}', got:\n{output}"
        );
    }
}

fn run_duckdb(sql: &str) -> String {
    let mut command = Command::new("build/release/duckdb");
    command.env("TZ", "UTC");
    run_duckdb_command(&mut command, sql)
}

fn run_duckdb_with_env(sql: &str, name: &str, value: &str) -> String {
    let mut command = Command::new("build/release/duckdb");
    command.env("TZ", "UTC");
    command.env(name, value);
    run_duckdb_command(&mut command, sql)
}

fn run_duckdb_command(command: &mut Command, sql: &str) -> String {
    let output = command
        .args(["-batch", "-csv", ":memory:", "-c", sql])
        .output()
        .expect("failed to run build/release/duckdb; run make release first");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "duckdb failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        stdout,
        stderr
    );
    format!("{stdout}{stderr}")
}

fn run_duckdb_command_after_setup(setup: &str, sql: &str) -> (bool, String) {
    let output = Command::new("build/release/duckdb")
        .args([
            "-batch", "-bail", "-csv", "-cmd", setup, ":memory:", "-c", sql,
        ])
        .output()
        .expect("failed to run build/release/duckdb; run make release first");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    (output.status.success(), format!("{stdout}{stderr}"))
}

fn extension_path() -> &'static str {
    let path = "build/release/extension/superhuman_docs/superhuman_docs.duckdb_extension";
    assert!(
        Path::new(path).exists(),
        "{path} does not exist; run make release first"
    );
    path
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
