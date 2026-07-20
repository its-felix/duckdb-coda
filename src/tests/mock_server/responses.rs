use super::*;

pub(super) fn mock_response(
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
