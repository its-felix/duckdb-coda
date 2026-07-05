import os
import subprocess
from dataclasses import dataclass


@dataclass
class TestFixture:
    name: str
    doc_id: str
    token: str
    api_base: str
    table_name: str
    state: object = None


def sql_quote(value):
    return str(value).replace("'", "''")


def sql_ident(value):
    return '"' + str(value).replace('"', '""') + '"'


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


def table_sql(fixture):
    return f"coda_doc.main.{sql_ident(fixture.table_name)}"
