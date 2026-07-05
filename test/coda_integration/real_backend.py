import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

from .glue import TestFixture


CODA_API_BASE = "https://coda.io/apis/v1"


def url_component(value):
    return urllib.parse.quote(str(value), safe="")


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


class RealBackend:
    def __init__(self, token, api_base=CODA_API_BASE, explicit_doc_id=None):
        self.api = RealCodaApi(token, api_base)
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
                return self
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
