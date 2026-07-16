use crate::attach::read_environment_variable;
use crate::ffi::*;
use crate::json::{
    column_list_from_json, free_coda_rows_response, free_columns, logical_type, rows_from_json,
};
use crate::mutation::{build_equality_query, insert_body};
use crate::sdk::{send_request, validate_token_at};
use serde_json::{json, Value};
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use superhuman_docs::{operations, Request, DEFAULT_BASE_URL};

#[test]
fn parse_columns_and_rows() {
    let columns = column_list_from_json(
        r#"{"items":[{"id":"c-id","name":"Amount","calculated":true,"format":{"type":"currency","isArray":false}}],"nextPageToken":"next"}"#,
    )
    .unwrap();
    assert_eq!(columns.count, 1);
    let column_items = slice_from_raw_parts(columns.items, columns.count);
    assert_eq!(column_items[0].id.as_str(), "c-id");
    assert_eq!(column_items[0].logical_type, RUST_EXT_LOGICAL_CURRENCY);
    free_columns(columns);

    let rows = rows_from_json(
        r#"{"items":[{"id":"r1","createdAt":"2024-01-01T00:00:00Z","values":{"c1":true,"c2":[1,2],"c3":"plain"}}],"nextSyncToken":"sync"}"#,
    )
    .unwrap();
    assert_eq!(rows.row_count, 1);
    let row_items = slice_from_raw_parts(rows.rows, rows.row_count);
    assert_eq!(row_items[0].cell_count, 3);
    assert_eq!(rows.next_sync_token.as_str(), "sync");
    free_coda_rows_response(rows);
}

#[test]
fn documented_column_formats_map_to_duckdb_logical_types() {
    for (format_type, expected) in [
        ("checkbox", RUST_EXT_LOGICAL_BOOLEAN),
        ("text", RUST_EXT_LOGICAL_VARCHAR),
        ("email", RUST_EXT_LOGICAL_VARCHAR),
        ("select", RUST_EXT_LOGICAL_VARCHAR),
        ("number", RUST_EXT_LOGICAL_DECIMAL),
        ("percent", RUST_EXT_LOGICAL_DECIMAL),
        ("slider", RUST_EXT_LOGICAL_DECIMAL),
        ("scale", RUST_EXT_LOGICAL_DECIMAL),
        ("date", RUST_EXT_LOGICAL_DATE),
        ("dateTime", RUST_EXT_LOGICAL_TIMESTAMP_TZ),
        ("time", RUST_EXT_LOGICAL_TIME),
        ("duration", RUST_EXT_LOGICAL_INTERVAL),
        ("currency", RUST_EXT_LOGICAL_CURRENCY),
        ("image", RUST_EXT_LOGICAL_IMAGE),
        ("person", RUST_EXT_LOGICAL_PERSON),
        ("link", RUST_EXT_LOGICAL_HYPERLINK),
        ("hyperlink", RUST_EXT_LOGICAL_HYPERLINK),
        ("lookup", RUST_EXT_LOGICAL_LOOKUP),
        ("canvas", RUST_EXT_LOGICAL_JSON),
    ] {
        assert_eq!(logical_type(format_type, false), expected, "{format_type}");
    }
    assert_eq!(logical_type("number", true), RUST_EXT_LOGICAL_DECIMAL);
    assert_eq!(logical_type("select", true), RUST_EXT_LOGICAL_VARCHAR);
}

#[test]
fn mutation_bodies_match_previous_shape() {
    let columns = [RustExtWriteColumn {
        id: alloc_string("c1"),
        capabilities: RUST_EXT_COLUMN_EDITABLE,
        ..Default::default()
    }];
    let values = [RustExtInputValue {
        value_type: 5,
        string_value: alloc_string("v"),
        ..Default::default()
    }];
    assert_eq!(
        insert_body(&columns, &values, 1, 1, RUST_EXT_TABLE_INSERT).unwrap(),
        r#"{"rows":[{"cells":[{"column":"c1","value":"v"}]}]}"#
    );
    columns[0].id.free();
    values[0].string_value.free();
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
    let column = RustExtColumn {
        id: alloc_string("createdAt"),
        capabilities: RUST_EXT_COLUMN_SORT_ASC,
        ..Default::default()
    };
    let mut sort_by = RustExtString::default();
    assert!(crate::exports::rust_ext_scan_sort_by(column, &mut sort_by));
    assert_eq!(sort_by.as_str(), "createdAt");
    assert_ne!(sort_by.ptr, column.id.ptr);
    sort_by.free();
    column.id.free();
}

#[test]
fn token_validation_uses_whoami_status() {
    let server = MockCodaServer::start();
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

    let server = MockCodaServer::start_with_whoami_status("401 Unauthorized");
    let error = validate_token_at(&server.base_url(), "bad-token").unwrap_err();
    assert_eq!(error, "Whoami returned HTTP 401, expected 200");
}

#[test]
fn token_environment_variable_is_read_eagerly() {
    let name = format!("DUCKDB_CODA_TOKEN_ENV_TEST_{}", std::process::id());
    env::set_var(&name, "resolved-token");
    assert_eq!(read_environment_variable(&name).unwrap(), "resolved-token");
    env::remove_var(&name);
    assert!(read_environment_variable(&name).is_err());
}

#[test]
#[ignore]
fn duckdb_mock_coda_scan_metadata_and_dml() {
    let server = MockCodaServer::start();
    let table = "coda_doc.main.\"Tasks\"";
    let sql = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS coda_doc (TYPE coda, TOKEN 'mock-token', API_BASE {}, INCLUDE_ROW_METADATA true);\
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
fn duckdb_mock_coda_token_env_for_attach() {
    let server = MockCodaServer::start();
    let env_name = "DUCKDB_CODA_MOCK_API_TOKEN";
    let sql = format!(
        "LOAD {};
         ATTACH 'mock-doc' AS coda_attach_env
             (TYPE coda, TOKEN_ENV {}, API_BASE {});
         SELECT count(*) FROM coda_attach_env.main.\"Tasks\";",
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
fn duckdb_mock_coda_wide_types() {
    let server = MockCodaServer::start();
    let table = "coda_doc.main.\"Wide Types\"";
    let sql = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS coda_doc (TYPE coda, TOKEN 'mock-token', API_BASE {});\
         SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_catalog = 'coda_doc' AND table_schema = 'main' AND table_name = 'Wide Types' \
         ORDER BY ordinal_position;\
         SELECT \"Checkbox\", \"Text\", \"Email\", \"Select\", \
                \"Number\", \"Percent\", \"Slider\", \"Scale\", \
                \"Date\", \"DateTime\", \"Time\", epoch(\"Duration\"), \
                \"Currency\".currency, \"Currency\".amount, \
                \"Image\".name, \"Image\".url, \"Image\".height, \"Image\".width, \"Image\".status, \
                \"Person\".name, \"Person\".email, \
                \"Hyperlink\".name, \"Hyperlink\".url, \
                \"Lookup\".name, \"Lookup\".url, \"Lookup\".tableId, \"Lookup\".tableUrl, \"Lookup\".rowId, \
                CAST(\"Other\" AS VARCHAR), CAST(\"MultiSelect\" AS VARCHAR), \
                list_transform(\"Durations\", value -> epoch(value)), \
                list_transform(\"Currencies\", value -> value.currency), CAST(\"Others\" AS VARCHAR) \
         FROM {table};",
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
}

#[test]
#[ignore]
fn duckdb_mock_coda_rejects_explicit_transactions() {
    let server = MockCodaServer::start();
    let setup = format!(
        "LOAD {};\
         ATTACH 'mock-doc' AS coda_doc (TYPE coda, TOKEN 'mock-token', API_BASE {});",
        sql_literal(extension_path()),
        sql_literal(&server.base_url())
    );
    let (success, output) = run_duckdb_command_after_setup(
        &setup,
        "BEGIN TRANSACTION; INSERT INTO coda_doc.main.\"Tasks\" (\"Name\", \"Done\", \"Amount\") VALUES ('Txn', false, 9.0); ROLLBACK;",
    );
    assert!(
        !success && output.contains("Coda does not support explicit DuckDB transactions"),
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
fn real_coda_api_smoke() {
    let credential = required_env("CODA_TEST_API_TOKEN");
    let endpoint = env::var("CODA_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = env::var("CODA_TEST_DOC_ID").unwrap_or_else(|_| {
        first_editable_doc(&endpoint, &credential).expect("no editable Coda doc found")
    });

    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let page_name = format!("duckdb-coda-test-{run_id}");
    let table_name = format!("duckdb_coda_test_{run_id}");
    let page = create_test_page(&endpoint, &credential, &resource, &page_name, &table_name)
        .expect("failed to create Coda test page");
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
            .expect("timed out waiting for generated Coda table");
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
fn real_coda_api_wide_types() {
    let credential = required_env("CODA_TEST_API_TOKEN");
    let endpoint = env::var("CODA_TEST_API_BASE")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let resource = required_env("CODA_TEST_DOC_ID");
    let configured_table = required_env("CODA_TEST_WIDE_TABLE_ID");
    let (table_id, table_name) =
        resolve_table_identity(&endpoint, &credential, &resource, &configured_table)
            .expect("could not find CODA_TEST_WIDE_TABLE_ID in CODA_TEST_DOC_ID");

    assert_wide_column_formats(&endpoint, &credential, &resource, &table_id)
        .expect("real Coda wide-table fixture has missing or incorrectly formatted columns");
    run_duckdb_real_wide_type_case(&resource, &credential, &endpoint, &table_name);
}

#[derive(Clone, Debug)]
struct MockRequest {
    method: String,
    path: String,
    query: String,
    headers: String,
    body: String,
}

struct MockCodaServer {
    address: String,
    requests: Arc<Mutex<Vec<MockRequest>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockCodaServer {
    fn start() -> Self {
        Self::start_with_whoami_status("200 OK")
    }

    fn start_with_whoami_status(whoami_status: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind mock Coda server");
        let address = listener
            .local_addr()
            .expect("failed to read mock Coda server address")
            .to_string();
        listener
            .set_nonblocking(true)
            .expect("failed to configure mock Coda server");
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
                Err(err) => panic!("mock Coda server failed to accept connection: {err}"),
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

impl Drop for MockCodaServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(handle) = self.handle.take() {
            handle.join().expect("mock Coda server thread panicked");
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
            .expect("failed to read mock Coda request");
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
            .expect("failed to read mock Coda request body");
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
    requests.lock().unwrap().push(MockRequest {
        method: method.clone(),
        path: path.clone(),
        query: query.clone(),
        headers,
        body: body.clone(),
    });

    let (status, response_body) = mock_response(&method, &path, &query, whoami_status);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_body.len(),
        response_body
    );
    stream
        .write_all(response.as_bytes())
        .expect("failed to write mock Coda response");
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn mock_response(
    method: &str,
    path: &str,
    query: &str,
    whoami_status: &'static str,
) -> (&'static str, String) {
    match (method, path) {
        ("GET", "/whoami") => (whoami_status, "not valid JSON".to_string()),
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
        ("POST", "/docs/mock-doc/tables/tbl1/rows")
        | ("PUT", "/docs/mock-doc/tables/tbl1/rows/r1")
        | ("DELETE", "/docs/mock-doc/tables/tbl1/rows") => ("202 Accepted", "{}".to_string()),
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
        let request = operations::build_delete_page(
            &self.endpoint,
            operations::DeletePageInput {
                doc_id: self.resource.clone(),
                page_id_or_name: self.page_id.clone(),
            },
        );
        if let Ok(request) = request {
            let _ = api_json(&self.endpoint, &self.credential, request);
        }
    }
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set in the environment"))
}

fn api_config(credential: &str) -> RustExtClientConfig {
    RustExtClientConfig {
        credential: alloc_string(credential),
        ..Default::default()
    }
}

fn api_json(_api_base: &str, credential: &str, request: Request) -> Result<Value, String> {
    let config = api_config(credential);
    let body = send_request(config, request);
    config.credential.free();
    let body = body?;
    if body.trim().is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(&body).map_err(|e| e.to_string())
    }
}

fn paged_items(
    endpoint: &str,
    credential: &str,
    mut build: impl FnMut(Option<String>) -> Result<Request, superhuman_docs::Error>,
) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut page_token = None;
    loop {
        let root = api_json(
            endpoint,
            credential,
            build(page_token.take()).map_err(|e| e.to_string())?,
        )?;
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

fn first_editable_doc(endpoint: &str, credential: &str) -> Result<String, String> {
    let docs = paged_items(endpoint, credential, |page_token| {
        operations::build_list_docs(
            endpoint,
            operations::ListDocsInput {
                limit: Some(100),
                page_token,
                ..Default::default()
            },
        )
    })?;
    docs.into_iter()
        .find(|doc| doc.get("canEdit").and_then(Value::as_bool).unwrap_or(false))
        .and_then(|doc| {
            doc.get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .ok_or_else(|| "no editable Coda doc found".to_string())
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
    let request = operations::build_create_page(
        endpoint,
        operations::CreatePageInput {
            doc_id: resource.to_string(),
            payload,
        },
    )
    .map_err(|e| e.to_string())?;
    api_json(endpoint, credential, request)
}

fn wait_for_page_table(
    endpoint: &str,
    credential: &str,
    resource: &str,
    page_id: &str,
    wanted_table_name: &str,
) -> Result<Value, String> {
    for _ in 0..40 {
        let tables = paged_items(endpoint, credential, |page_token| {
            operations::build_list_tables(
                endpoint,
                operations::ListTablesInput {
                    doc_id: resource.to_string(),
                    limit: Some(100),
                    page_token,
                    ..Default::default()
                },
            )
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
                        "Coda named integration table '{table_name}'; requested '{wanted_table_name}'"
                    );
                }
                return Ok(table.clone());
            }
        }
        thread::sleep(Duration::from_secs(3));
    }
    Err("timed out waiting for Coda table".to_string())
}

fn assert_required_columns(
    endpoint: &str,
    credential: &str,
    resource: &str,
    table_id: &str,
) -> Result<(), String> {
    let columns = paged_items(endpoint, credential, |page_token| {
        operations::build_list_columns(
            endpoint,
            operations::ListColumnsInput {
                doc_id: resource.to_string(),
                table_id_or_name: table_id.to_string(),
                limit: Some(100),
                page_token,
                visible_only: Some(false),
            },
        )
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

fn assert_wide_column_formats(
    endpoint: &str,
    credential: &str,
    resource: &str,
    table_id: &str,
) -> Result<(), String> {
    let columns = paged_items(endpoint, credential, |page_token| {
        operations::build_list_columns(
            endpoint,
            operations::ListColumnsInput {
                doc_id: resource.to_string(),
                table_id_or_name: table_id.to_string(),
                limit: Some(100),
                page_token,
                visible_only: Some(false),
            },
        )
    })?;
    for (name, format_type, expected_array) in [
        ("Checkbox", "checkbox", false),
        ("Text", "text", false),
        ("Email", "email", false),
        ("Select", "select", false),
        ("Number", "number", false),
        ("Percent", "percent", false),
        ("Slider", "slider", false),
        ("Scale", "scale", false),
        ("Date", "date", false),
        ("DateTime", "dateTime", false),
        ("Time", "time", false),
        ("Duration", "duration", false),
        ("Currency", "currency", false),
        ("Image", "image", false),
        ("Person", "person", false),
        ("Hyperlink", "link", false),
        ("Lookup", "lookup", false),
        ("Other", "canvas", false),
        ("Durations", "duration", true),
    ] {
        let actual = columns
            .iter()
            .find(|column| column.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|column| column.get("format"))
            .ok_or_else(|| format!("missing required wide-table column {name}"))?;
        let actual_type = actual.get("type").and_then(Value::as_str).unwrap_or("");
        let is_array = actual
            .get("isArray")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if actual_type != format_type || is_array != expected_array {
            return Err(format!(
                "column {name} must have format {format_type} with isArray={expected_array}, got type={actual_type}, isArray={is_array}"
            ));
        }
    }
    Ok(())
}

fn resolve_table_identity(
    endpoint: &str,
    credential: &str,
    resource: &str,
    configured_table: &str,
) -> Result<(String, String), String> {
    let tables = paged_items(endpoint, credential, |page_token| {
        operations::build_list_tables(
            endpoint,
            operations::ListTablesInput {
                doc_id: resource.to_string(),
                limit: Some(100),
                page_token,
                ..Default::default()
            },
        )
    })?;
    tables
        .iter()
        .find(|table| {
            table.get("id").and_then(Value::as_str) == Some(configured_table)
                || table.get("name").and_then(Value::as_str) == Some(configured_table)
        })
        .and_then(|table| {
            Some((
                table.get("id")?.as_str()?.to_string(),
                table.get("name")?.as_str()?.to_string(),
            ))
        })
        .ok_or_else(|| format!("table {configured_table} was not found"))
}

fn run_duckdb_real_wide_type_case(
    resource: &str,
    credential: &str,
    endpoint: &str,
    table_name: &str,
) {
    let table = format!("coda_doc.main.{}", sql_ident(table_name));
    let sql = format!(
        "LOAD {};\
         ATTACH {} AS coda_doc (TYPE coda, TOKEN {}, API_BASE {});\
         SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_catalog = 'coda_doc' AND table_schema = 'main' AND table_name = {} \
         ORDER BY ordinal_position;\
         SELECT count(*) > 0 AS has_populated_wide_row FROM {table} \
         WHERE \"Checkbox\" IS NOT NULL AND \"Text\" IS NOT NULL AND \"Email\" IS NOT NULL \
           AND \"Select\" IS NOT NULL AND \"Number\" IS NOT NULL AND \"Percent\" IS NOT NULL \
           AND \"Slider\" IS NOT NULL AND \"Scale\" IS NOT NULL AND \"Date\" IS NOT NULL \
           AND \"DateTime\" IS NOT NULL AND \"Time\" IS NOT NULL AND \"Duration\" IS NOT NULL \
           AND \"Currency\".currency IS NOT NULL AND \"Currency\".amount IS NOT NULL \
           AND \"Image\".name IS NOT NULL AND \"Image\".url IS NOT NULL \
           AND \"Image\".height IS NOT NULL AND \"Image\".width IS NOT NULL AND \"Image\".status IS NOT NULL \
           AND \"Person\".name IS NOT NULL AND \"Person\".email IS NOT NULL \
           AND \"Hyperlink\".name IS NOT NULL AND \"Hyperlink\".url IS NOT NULL \
           AND \"Lookup\".name IS NOT NULL AND \"Lookup\".url IS NOT NULL \
           AND \"Lookup\".tableId IS NOT NULL AND \"Lookup\".tableUrl IS NOT NULL \
           AND \"Lookup\".rowId IS NOT NULL AND \"Other\" IS NOT NULL \
           AND len(\"Durations\") > 0 AND \"Durations\"[1] IS NOT NULL;",
        sql_literal(extension_path()),
        sql_literal(resource),
        sql_literal(credential),
        sql_literal(endpoint),
        sql_literal(table_name),
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
        "Durations,INTERVAL[]",
        "has_populated_wide_row\ntrue",
    ] {
        assert!(
            output.contains(expected),
            "expected real wide-type output to contain '{expected}', got:\n{output}"
        );
    }
}

fn run_duckdb_success_case(resource: &str, credential: &str, endpoint: &str, table_name: &str) {
    let table = format!("coda_doc.main.{}", sql_ident(table_name));
    let sql = format!(
        "LOAD {};\
         ATTACH {} AS coda_doc (TYPE coda, TOKEN {}, API_BASE {});\
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
    let table = format!("coda_doc.main.{}", sql_ident(table_name));
    let sql = format!(
        "LOAD {};\
         ATTACH {} AS coda_doc (TYPE coda, TOKEN {}, API_BASE {}, INCLUDE_ROW_METADATA true);\
         SELECT column_name, data_type \
         FROM information_schema.columns \
         WHERE table_catalog = 'coda_doc' \
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
    run_duckdb_command(&mut Command::new("build/release/duckdb"), sql)
}

fn run_duckdb_with_env(sql: &str, name: &str, value: &str) -> String {
    let mut command = Command::new("build/release/duckdb");
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
    let path = "build/release/extension/coda/coda.duckdb_extension";
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
