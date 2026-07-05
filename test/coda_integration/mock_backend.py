import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from .glue import TestFixture, attach_sql, run_duckdb


MOCK_DOC_ID = "test-doc"
MOCK_TOKEN = "test-token"


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
            items = [
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
            query = parse_qs(parsed.query, keep_blank_values=True)
            if "syncToken" in query:
                self._send_json({"items": [], "nextSyncToken": "sync-token-2"})
                return
            coda_query = query.get("query", [])
            if coda_query:
                column, _, raw_value = coda_query[-1].partition(":")
                value = json.loads(raw_value)
                items = [item for item in items if item["values"].get(column) == value]
            sort_by = query.get("sortBy", [])
            if sort_by and sort_by[-1] in ("createdAt", "updatedAt"):
                items = sorted(items, key=lambda item: item[sort_by[-1]])
            limit = int(query.get("limit", ["500"])[-1])
            self._send_json(
                {
                    "items": items[:limit],
                    "nextSyncToken": "sync-token-1",
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


class MockBackend:
    def __enter__(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), MockCodaHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.fixture = TestFixture(
            name="mock",
            doc_id=MOCK_DOC_ID,
            token=MOCK_TOKEN,
            api_base=f"http://127.0.0.1:{self.server.server_port}",
            table_name="Tasks",
            state=MockCodaHandler.state,
        )
        return self

    def __exit__(self, exc_type, exc, tb):
        self.server.shutdown()
        self.server.server_close()

    def clear_requests(self):
        MockCodaHandler.state.clear()

    def assert_success_case(self):
        requests = MockCodaHandler.state.snapshot()
        assert_mock_authenticated(requests)

        tables = require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables")
        assert_query(tables["query"], "limit", ["100"])

        columns = require_request(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/columns")
        assert_query(columns["query"], "limit", ["100"])
        assert_query(columns["query"], "visibleOnly", ["false"])

        row_requests = request_matching(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")
        rows = next(row for row in row_requests if "query" not in row["query"])
        assert_query(rows["query"], "valueFormat", ["simpleWithArrays"])
        assert_query(rows["query"], "useColumnNames", ["false"])
        assert_query(rows["query"], "visibleOnly", ["false"])
        assert_query(rows["query"], "limit", ["500"])
        alpha_rows = next(row for row in row_requests if row["query"].get("query") == ['c-name:"Alpha"'])
        assert_query(alpha_rows["query"], "limit", ["500"])
        beta_rows = next(row for row in row_requests if row["query"].get("query") == ['c-name:"Beta"'])
        assert_query(beta_rows["query"], "limit", ["500"])
        if not any(row["query"].get("syncToken") == ["sync-token-1"] for row in row_requests):
            raise AssertionError(f"missing syncToken rows request; saw {row_requests}")

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

    def assert_metadata_case(self):
        requests = MockCodaHandler.state.snapshot()
        assert_mock_authenticated(requests)
        rows = request_matching(requests, "GET", f"/docs/{MOCK_DOC_ID}/tables/grid-1/rows")
        if not any(row["query"].get("query") == ['c-name:"Alpha"'] for row in rows):
            raise AssertionError(f"missing pushed metadata filter request; saw {rows}")
        sorted_rows = [row for row in rows if row["query"].get("sortBy") == ["createdAt"]]
        if not sorted_rows:
            raise AssertionError(f"missing pushed metadata sort request; saw {rows}")
        assert_query(sorted_rows[-1]["query"], "limit", ["1"])

    def run_failure_case(self, duckdb, extension, prefix, expected_error):
        result = run_duckdb(
            duckdb,
            extension,
            f"{attach_sql(self.fixture, api_prefix=prefix)}\n",
            expect_success=False,
        )
        combined = result.stdout + result.stderr
        if expected_error not in combined:
            raise AssertionError(
                f"expected failure containing {expected_error!r}, got:\n"
                f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
            )


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
