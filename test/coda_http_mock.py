#!/usr/bin/env python3

import argparse
import json
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


DOC_ID = "test-doc"
TOKEN = "test-token"


def sql_quote(value):
    return str(value).replace("'", "''")


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
        return self.headers.get("Authorization") == f"Bearer {TOKEN}"

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

        if self.command == "GET" and parsed.path == f"/docs/{DOC_ID}/tables":
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

        if self.command == "GET" and parsed.path == f"/docs/{DOC_ID}/tables/grid-1/columns":
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

        if self.command == "GET" and parsed.path == f"/docs/{DOC_ID}/tables/grid-1/rows":
            self._send_json(
                {
                    "items": [
                        {
                            "id": "row-1",
                            "values": {
                                "c-name": "Alpha",
                                "c-done": True,
                                "c-amount": 1.25,
                                "c-formula": "computed",
                            },
                        },
                        {
                            "id": "row-2",
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

        if self.command == "POST" and parsed.path == f"/docs/{DOC_ID}/tables/grid-1/rows":
            self._send_json({"requestId": "insert-request"}, 202)
            return

        if self.command == "PUT" and parsed.path == f"/docs/{DOC_ID}/tables/grid-1/rows/row-1":
            self._send_json({"requestId": "update-request"}, 202)
            return

        if self.command == "DELETE" and parsed.path == f"/docs/{DOC_ID}/tables/grid-1/rows/row-2":
            self._send_json({"requestId": "delete-request"}, 202)
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


def attach_sql(api_base):
    return f"ATTACH '{DOC_ID}' AS coda_doc " f"(TYPE coda, TOKEN '{TOKEN}', API_BASE '{sql_quote(api_base)}');"


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


def assert_authenticated(requests):
    for request in requests:
        authorization = request["headers"].get("authorization")
        if authorization != f"Bearer {TOKEN}":
            raise AssertionError(f"bad authorization header on {request}: {authorization}")
        accept = request["headers"].get("accept")
        if accept != "application/json":
            raise AssertionError(f"bad accept header on {request}: {accept}")


def assert_json_cells(actual_cells, expected):
    by_column = {cell["column"]: cell.get("value") for cell in actual_cells}
    if by_column != expected:
        raise AssertionError(f"expected cells {expected}, got {by_column}")


def run_success_case(duckdb, extension, api_base, state):
    state.clear()
    sql = f"""
{attach_sql(api_base)}
SELECT Name, Done, Amount FROM coda_doc.main.Tasks ORDER BY Name;
INSERT INTO coda_doc.main.Tasks (Name, Done, Amount)
VALUES ('Gamma', false, 3.5);
UPDATE coda_doc.main.Tasks SET Done = false, Amount = 4.5 WHERE Name = 'Alpha';
DELETE FROM coda_doc.main.Tasks WHERE Name = 'Beta';
"""
    result = run_duckdb(duckdb, extension, sql)
    if "Alpha" not in result.stdout or "Beta" not in result.stdout:
        raise AssertionError(f"expected SELECT output to include mocked rows, got:\n{result.stdout}")

    requests = state.snapshot()
    assert_authenticated(requests)

    tables = require_request(requests, "GET", f"/docs/{DOC_ID}/tables")
    assert_query(tables["query"], "limit", ["100"])

    columns = require_request(requests, "GET", f"/docs/{DOC_ID}/tables/grid-1/columns")
    assert_query(columns["query"], "limit", ["100"])
    assert_query(columns["query"], "visibleOnly", ["false"])

    rows = require_request(requests, "GET", f"/docs/{DOC_ID}/tables/grid-1/rows")
    assert_query(rows["query"], "valueFormat", ["simpleWithArrays"])
    assert_query(rows["query"], "useColumnNames", ["false"])
    assert_query(rows["query"], "visibleOnly", ["false"])
    assert_query(rows["query"], "limit", ["500"])

    insert = require_request(requests, "POST", f"/docs/{DOC_ID}/tables/grid-1/rows")
    assert_query(insert["query"], "disableParsing", ["false"])
    insert_body = json.loads(insert["body"])
    assert_json_cells(
        insert_body["rows"][0]["cells"],
        {"c-name": "Gamma", "c-done": False, "c-amount": 3.5},
    )

    update = require_request(requests, "PUT", f"/docs/{DOC_ID}/tables/grid-1/rows/row-1")
    assert_query(update["query"], "disableParsing", ["false"])
    update_body = json.loads(update["body"])
    assert_json_cells(update_body["row"]["cells"], {"c-done": False, "c-amount": 4.5})

    delete = require_request(requests, "DELETE", f"/docs/{DOC_ID}/tables/grid-1/rows/row-2")
    if delete["body"]:
        raise AssertionError(f"DELETE should not send a request body, got {delete['body']}")


def run_failure_case(duckdb, extension, api_base, prefix, expected_error):
    result = run_duckdb(
        duckdb,
        extension,
        f"{attach_sql(api_base + prefix)}\n",
        expect_success=False,
    )
    combined = result.stdout + result.stderr
    if expected_error not in combined:
        raise AssertionError(
            f"expected failure containing {expected_error!r}, got:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--duckdb", default="build/debug/duckdb")
    parser.add_argument("--extension", default="build/debug/extension/coda/coda.duckdb_extension")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    duckdb = (root / args.duckdb).resolve()
    extension = (root / args.extension).resolve()
    if not duckdb.exists():
        raise AssertionError(f"DuckDB binary does not exist: {duckdb}")
    if not extension.exists():
        raise AssertionError(f"Coda extension does not exist: {extension}")

    server = ThreadingHTTPServer(("127.0.0.1", 0), MockCodaHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    api_base = f"http://127.0.0.1:{server.server_port}"

    try:
        run_success_case(duckdb, extension, api_base, MockCodaHandler.state)
        run_failure_case(duckdb, extension, api_base, "/status500", "HTTP 500")
        run_failure_case(duckdb, extension, api_base, "/invalid-json", "Failed to parse JSON")
        run_failure_case(duckdb, extension, api_base, "/empty-body", "Failed to parse JSON")
        run_failure_case(
            duckdb,
            extension,
            api_base,
            "/missing-items",
            "missing array member 'items'",
        )
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"coda_http_mock.py: {exc}", file=sys.stderr)
        sys.exit(1)
