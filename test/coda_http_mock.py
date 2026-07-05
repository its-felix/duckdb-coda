#!/usr/bin/env python3

import argparse
import json
import os
import subprocess
import sys
import threading
import time
import uuid
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


MOCK_DOC_ID = "test-doc"
MOCK_TOKEN = "test-token"
CODA_API_BASE = "https://coda.io/apis/v1"


def sql_quote(value):
    return str(value).replace("'", "''")


def sql_ident(value):
    return '"' + str(value).replace('"', '""') + '"'


def url_component(value):
    return urllib.parse.quote(str(value), safe="")


def load_dotenv(root):
    env_path = root / ".env"
    if not env_path.exists():
        return
    for raw_line in env_path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        if key in os.environ:
            continue
        os.environ[key] = value.strip().strip('"').strip("'")


class MockCodaState:
    def __init__(self):
        self.lock = threading.Lock()
        self.requests = []

    def append(self, request):
        with self.lock:
            self.requests.append(request)

    def snapshot(self):
        with self.lock:
            return list(self.requests)

    def clear(self):
        with self.lock:
            self.requests.clear()


class MockCodaHandler(BaseHTTPRequestHandler):
    state = MockCodaState()

    def log_message(self, fmt, *args):
        return

    def _record(self, body):
        parsed = urlparse(self.path)
        request = {
            "method": self.command,
            "path": parsed.path,
            "query": parse_qs(parsed.query, keep_blank_values=True),
            "headers": {key.lower(): value for key, value in self.headers.items()},
            "body": body.decode("utf-8"),
        }
        self.state.append(request)
        return parsed

    def _send_json(self, value, status=200):
        payload = json.dumps(value).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def _send_text(self, value, status=200):
        payload = value.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def _send_empty(self, status=200):
        self.send_response(status)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _authorized(self):
        return self.headers.get("Authorization") == f"Bearer {MOCK_TOKEN}"

    def _handle(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length else b""
        parsed = self._record(body)

        if parsed.path.startswith("/status500/"):
            self._send_json({"message": "forced failure"}, 500)
            return
        if parsed.path.startswith("/invalid-json/"):
            self._send_text("not-json")
            return
        if parsed.path.startswith("/empty-body/"):
            self._send_empty()
            return
        if parsed.path.startswith("/missing-items/"):
            self._send_json({})
            return
        if not self._authorized():
            self._send_json({"message": "missing auth"}, 401)
            return

        if self.command == "GET" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables":
            self._send_json(
                {
                    "items": [
                        {
                            "id": "grid-1",
                            "name": "Tasks",
                            "tableType": "table",
                        }
                    ]
                }
            )
            return

        if self.command == "GET" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables/grid-1/columns":
            self._send_json(
                {
                    "items": [
                        {"id": "c-name", "name": "Name", "format": {"type": "text"}},
                        {
                            "id": "c-done",
                            "name": "Done",
                            "format": {"type": "checkbox"},
                        },
                        {
                            "id": "c-amount",
                            "name": "Amount",
                            "format": {"type": "number"},
                        },
                        {
                            "id": "c-formula",
                            "name": "Formula",
                            "format": {"type": "text"},
                            "calculated": True,
                        },
                    ]
                }
            )
            return

        if self.command == "GET" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows":
            self._send_json(
                {
                    "items": [
                        {
                            "id": "row-1",
                            "createdAt": "2018-04-11T00:18:57.946Z",
                            "updatedAt": "2018-04-12T00:18:57.946Z",
                            "values": {
                                "c-name": "Alpha",
                                "c-done": True,
                                "c-amount": 1.25,
                                "c-formula": "computed",
                            },
                        },
                        {
                            "id": "row-2",
                            "createdAt": "2018-04-13T00:18:57.946Z",
                            "updatedAt": "2018-04-14T00:18:57.946Z",
                            "values": {
                                "c-name": "Beta",
                                "c-done": False,
                                "c-amount": 2.5,
                                "c-formula": "computed",
                            },
                        },
                    ]
                }
            )
            return

        if self.command == "POST" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows":
            self._send_json({"requestId": "insert-request"}, 202)
            return

        if self.command == "PUT" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows/row-1":
            self._send_json({"requestId": "update-request"}, 202)
            return

        if self.command == "DELETE" and parsed.path == f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows":
            self._send_json({"requestId": "delete-request", "rowIds": ["row-2"]}, 202)
            return

        self._send_json({"message": f"unexpected {self.command} {parsed.path}"}, 404)

    def do_GET(self):
        self._handle()

    def do_POST(self):
        self._handle()

    def do_PUT(self):
        self._handle()

    def do_DELETE(self):
        self._handle()


@dataclass
class TestFixture:
    name: str
    doc_id: str
    token: str
    api_base: str
    table_name: str
    state: MockCodaState = None


def run_duckdb(duckdb, extension, sql, expect_success=True):
    result = subprocess.run(
        [str(duckdb), "-batch", "-csv", ":memory:"],
        input=f"LOAD '{sql_quote(extension)}';\n{sql}",
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if expect_success and result.returncode != 0:
        raise AssertionError(
            f"duckdb failed unexpectedly with code {result.returncode}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    if not expect_success and result.returncode == 0:
        raise AssertionError(f"duckdb succeeded unexpectedly\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}")
    if result.returncode < 0:
        raise AssertionError(
            f"duckdb process crashed with signal {-result.returncode}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def attach_sql(fixture, include_row_metadata=False, api_prefix=""):
    options = [
        "TYPE coda",
        f"TOKEN '{sql_quote(fixture.token)}'",
        f"API_BASE '{sql_quote(fixture.api_base + api_prefix)}'",
    ]
    if include_row_metadata:
        options.append("INCLUDE_ROW_METADATA true")
    return f"ATTACH '{sql_quote(fixture.doc_id)}' AS coda_doc ({', '.join(options)});"


def request_matching(requests, method, path):
    return [request for request in requests if request["method"] == method and request["path"] == path]


def require_request(requests, method, path):
    matches = request_matching(requests, method, path)
    if not matches:
        raise AssertionError(f"missing request {method} {path}; saw {requests}")
    return matches[-1]


def assert_query(query, key, expected):
    actual = query.get(key)
    if actual != expected:
        raise AssertionError(f"expected query parameter {key}={expected}, got {actual}")


def assert_mock_authenticated(requests):
    for request in requests:
        authorization = request["headers"].get("authorization")
        if authorization != f"Bearer {MOCK_TOKEN}":
            raise AssertionError(f"bad authorization header on {request}: {authorization}")
        accept = request["headers"].get("accept")
        if accept != "application/json":
            raise AssertionError(f"bad accept header on {request}: {accept}")


def assert_json_cells(actual_cells, expected):
    by_column = {cell["column"]: cell.get("value") for cell in actual_cells}
    if by_column != expected:
        raise AssertionError(f"expected cells {expected}, got {by_column}")


def table_sql(fixture):
    return f"coda_doc.main.{sql_ident(fixture.table_name)}"


def run_success_case(duckdb, extension, fixture):
    if fixture.state:
        fixture.state.clear()
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

    if not fixture.state:
        return

    requests = fixture.state.snapshot()
    assert_mock_authenticated(requests)

    tables = require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables")
    assert_query(tables["query"], "limit", ["100"])

    columns = require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/columns")
    assert_query(columns["query"], "limit", ["100"])
    assert_query(columns["query"], "visibleOnly", ["false"])

    rows = require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")
    assert_query(rows["query"], "valueFormat", ["simpleWithArrays"])
    assert_query(rows["query"], "useColumnNames", ["false"])
    assert_query(rows["query"], "visibleOnly", ["false"])
    assert_query(rows["query"], "limit", ["500"])

    insert = require_request(requests, "POST", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")
    assert_query(insert["query"], "disableParsing", ["false"])
    insert_body = json.loads(insert["body"])
    assert_json_cells(
        insert_body["rows"][0]["cells"],
        {"c-name": "Gamma", "c-done": False, "c-amount": 3.5},
    )

    update = require_request(requests, "PUT", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows/row-1")
    assert_query(update["query"], "disableParsing", ["false"])
    update_body = json.loads(update["body"])
    assert_json_cells(update_body["row"]["cells"], {"c-done": False, "c-amount": 4.5})

    delete = require_request(requests, "DELETE", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")
    delete_body = json.loads(delete["body"])
    if delete_body != {"rowIds": ["row-2"]}:
        raise AssertionError(f"expected bulk DELETE rowIds body, got {delete_body}")


def run_metadata_case(duckdb, extension, fixture):
    if fixture.state:
        fixture.state.clear()
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

    if fixture.state:
        requests = fixture.state.snapshot()
        assert_mock_authenticated(requests)
        require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")


def run_failure_case(duckdb, extension, fixture, prefix, expected_error):
    result = run_duckdb(
        duckdb,
        extension,
        f"{attach_sql(fixture, api_prefix=prefix)}\n",
        expect_success=False,
    )
    combined = result.stdout + result.stderr
    if expected_error not in combined:
        raise AssertionError(
            f"expected failure containing {expected_error!r}, got:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


class CodaApiError(Exception):
    def __init__(self, method, path, status, body):
        super().__init__(f"{method} {path} failed with HTTP {status}: {body[:500]}")
        self.method = method
        self.path = path
        self.status = status
        self.body = body


class RealCodaApi:
    def __init__(self, token, api_base=CODA_API_BASE):
        self.token = token
        self.api_base = api_base.rstrip("/")

    def request(self, method, path, body=None, expected=(200, 202)):
        payload = None
        headers = {
            "Authorization": f"Bearer {self.token}",
            "Accept": "application/json",
        }
        if body is not None:
            payload = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(
            self.api_base + path,
            data=payload,
            headers=headers,
            method=method,
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                response_body = response.read().decode("utf-8")
                status = response.status
        except urllib.error.HTTPError as exc:
            response_body = exc.read().decode("utf-8", errors="replace")
            raise CodaApiError(method, path, exc.code, response_body) from exc
        if status not in expected:
            raise CodaApiError(method, path, status, response_body)
        return json.loads(response_body) if response_body else {}

    def list_all(self, path):
        items = []
        page_token = None
        while True:
            separator = "&" if "?" in path else "?"
            page_path = path
            if page_token:
                page_path += f"{separator}pageToken={url_component(page_token)}"
            response = self.request("GET", page_path, expected=(200,))
            items.extend(response.get("items", []))
            page_token = response.get("nextPageToken")
            if not page_token:
                return items

    def list_docs(self):
        return self.list_all("/docs?limit=100")

    def create_page(self, doc_id, page_name, table_name):
        html = f"""
<h1>{table_name}</h1>
<table>
  <caption>{table_name}</caption>
  <thead>
    <tr><th>Name</th><th>Done</th><th>Amount</th></tr>
  </thead>
  <tbody>
    <tr><td>Alpha</td><td>true</td><td>1.25</td></tr>
    <tr><td>Beta</td><td>false</td><td>2.5</td></tr>
  </tbody>
</table>
"""
        body = {
            "name": page_name,
            "pageContent": {
                "type": "canvas",
                "canvasContent": {"format": "html", "content": html},
            },
        }
        return self.request("POST", f"/docs/{url_component(doc_id)}/pages", body=body, expected=(202,))

    def delete_page(self, doc_id, page_id):
        try:
            self.request("DELETE", f"/docs/{url_component(doc_id)}/pages/{url_component(page_id)}", expected=(202,))
        except CodaApiError as exc:
            if exc.status not in (404, 410):
                raise

    def list_tables(self, doc_id):
        return self.list_all(f"/docs/{url_component(doc_id)}/tables?limit=100")

    def list_columns(self, doc_id, table_id):
        return self.list_all(
            f"/docs/{url_component(doc_id)}/tables/{url_component(table_id)}/columns?limit=100&visibleOnly=false"
        )


class RealCodaFixture:
    def __init__(self, api, explicit_doc_id=None):
        self.api = api
        self.explicit_doc_id = explicit_doc_id
        self.page_id = None
        self.doc_id = None
        self.fixture = None

    def __enter__(self):
        run_id = uuid.uuid4().hex[:10]
        page_name = f"duckdb-coda-test-{run_id}"
        wanted_table_name = f"duckdb_coda_test_{run_id}"
        docs = self._candidate_docs()
        if not docs:
            raise AssertionError("Coda API returned no editable docs for CODA_TEST_API_TOKEN")

        last_error = None
        for doc in docs:
            doc_id = doc["id"]
            try:
                created = self.api.create_page(doc_id, page_name, wanted_table_name)
                self.doc_id = doc_id
                self.page_id = created["id"]
                table = self._wait_for_page_table(doc_id, self.page_id, wanted_table_name)
                self._assert_required_columns(doc_id, table["id"])
                self.fixture = TestFixture(
                    name="real",
                    doc_id=doc_id,
                    token=self.api.token,
                    api_base=self.api.api_base,
                    table_name=table["name"],
                )
                return self.fixture
            except Exception as exc:
                last_error = exc
                if self.page_id and self.doc_id:
                    self.api.delete_page(self.doc_id, self.page_id)
                self.page_id = None
                self.doc_id = None
                if self.explicit_doc_id:
                    break
        raise AssertionError(f"failed to create a usable Coda integration-test page: {last_error}") from last_error

    def __exit__(self, exc_type, exc, tb):
        if self.page_id and self.doc_id:
            self.api.delete_page(self.doc_id, self.page_id)

    def _candidate_docs(self):
        if self.explicit_doc_id:
            return [{"id": self.explicit_doc_id, "name": self.explicit_doc_id, "canEdit": True}]
        docs = self.api.list_docs()
        return [doc for doc in docs if doc.get("canEdit")]

    def _wait_for_page_table(self, doc_id, page_id, wanted_table_name):
        deadline = time.monotonic() + 120
        last_tables = []
        while time.monotonic() < deadline:
            last_tables = self.api.list_tables(doc_id)
            page_tables = [
                table
                for table in last_tables
                if table.get("tableType", "table") == "table"
                and table.get("parent", {}).get("id") == page_id
            ]
            if page_tables:
                table = page_tables[0]
                same_name_count = sum(1 for candidate in last_tables if candidate.get("name") == table.get("name"))
                if same_name_count > 1:
                    raise AssertionError(
                        f"Coda created table {table.get('name')!r}, but that name is not unique in the doc"
                    )
                if table.get("name") != wanted_table_name:
                    print(
                        f"coda_http_mock.py: Coda named integration table {table.get('name')!r}; "
                        f"requested {wanted_table_name!r}",
                        file=sys.stderr,
                    )
                return table
            time.sleep(3)
        raise AssertionError(f"timed out waiting for a table on page {page_id}; saw tables: {last_tables}")

    def _assert_required_columns(self, doc_id, table_id):
        columns = {column.get("name") for column in self.api.list_columns(doc_id, table_id)}
        required = {"Name", "Done", "Amount"}
        if not required.issubset(columns):
            raise AssertionError(f"Coda-created table is missing required columns {required - columns}; saw {columns}")


def run_mock_suite(duckdb, extension):
    server = ThreadingHTTPServer(("127.0.0.1", 0), MockCodaHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    fixture = TestFixture(
        name="mock",
        doc_id=MOCK_DOC_ID,
        token=MOCK_TOKEN,
        api_base=f"http://127.0.0.1:{server.server_port}",
        table_name="Tasks",
        state=MockCodaHandler.state,
    )

    try:
        run_success_case(duckdb, extension, fixture)
        run_metadata_case(duckdb, extension, fixture)
        run_failure_case(duckdb, extension, fixture, "/status500", "HTTP 500")
        run_failure_case(duckdb, extension, fixture, "/invalid-json", "Failed to parse JSON")
        run_failure_case(duckdb, extension, fixture, "/empty-body", "Failed to parse JSON")
        run_failure_case(
            duckdb,
            extension,
            fixture,
            "/missing-items",
            "missing array member 'items'",
        )
    finally:
        server.shutdown()
        server.server_close()


def run_real_suite(duckdb, extension, require_real):
    token = os.environ.get("CODA_TEST_API_TOKEN")
    if not token:
        message = "CODA_TEST_API_TOKEN is not set; skipping real Coda integration tests"
        if require_real:
            raise AssertionError(message)
        print(f"coda_http_mock.py: {message}", file=sys.stderr)
        return

    api = RealCodaApi(token, os.environ.get("CODA_TEST_API_BASE", CODA_API_BASE))
    explicit_doc_id = os.environ.get("CODA_TEST_DOC_ID")
    with RealCodaFixture(api, explicit_doc_id) as fixture:
        run_success_case(duckdb, extension, fixture)
        run_metadata_case(duckdb, extension, fixture)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--duckdb", default="build/debug/duckdb")
    parser.add_argument("--extension", default="build/debug/extension/coda/coda.duckdb_extension")
    parser.add_argument("--backend", choices=("mock", "real", "both"), default="mock")
    parser.add_argument("--require-real", action="store_true")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    load_dotenv(root)
    duckdb = (root / args.duckdb).resolve()
    extension = (root / args.extension).resolve()
    if not duckdb.exists():
        raise AssertionError(f"DuckDB binary does not exist: {duckdb}")
    if not extension.exists():
        raise AssertionError(f"Coda extension does not exist: {extension}")

    if args.backend in ("mock", "both"):
        run_mock_suite(duckdb, extension)
    if args.backend in ("real", "both"):
        run_real_suite(duckdb, extension, args.require_real)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"coda_http_mock.py: {exc}", file=sys.stderr)
        sys.exit(1)
