import os
import sys

from .mock_backend import MockBackend
from .real_backend import CODA_API_BASE, RealBackend
from .testcases import run_metadata_case, run_success_case


def run_mock_suite(duckdb, extension):
    with MockBackend() as backend:
        backend.clear_requests()
        run_success_case(duckdb, extension, backend.fixture)
        backend.assert_success_case()

        backend.clear_requests()
        run_metadata_case(duckdb, extension, backend.fixture)
        backend.assert_metadata_case()

        for prefix, expected_error in (
            ("/status500", "HTTP 500"),
            ("/invalid-json", "Failed to parse JSON"),
            ("/empty-body", "Failed to parse JSON"),
            ("/missing-items", "missing array member 'items'"),
        ):
            backend.run_failure_case(duckdb, extension, prefix, expected_error)


def run_real_suite(duckdb, extension, require_real):
    token = os.environ.get("CODA_TEST_API_TOKEN")
    if not token:
        message = "CODA_TEST_API_TOKEN is not set; skipping real Coda integration tests"
        if require_real:
            raise AssertionError(message)
        print(f"coda_http_mock.py: {message}", file=sys.stderr)
        return

    api_base = os.environ.get("CODA_TEST_API_BASE", CODA_API_BASE)
    explicit_doc_id = os.environ.get("CODA_TEST_DOC_ID")
    with RealBackend(token, api_base, explicit_doc_id) as backend:
        run_success_case(duckdb, extension, backend.fixture)
        run_metadata_case(duckdb, extension, backend.fixture)
