#!/usr/bin/env python3

import argparse
import sys
from pathlib import Path

from coda_integration.glue import load_dotenv
from coda_integration.harness import run_mock_suite, run_real_suite


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
