"""Export a table via zero-copy Apache Arrow and query it lazily with DuckDB.

Usage:
    python examples/arrow_query.py database.db plants
"""

from __future__ import annotations

import sys

import duckdb

import sa17


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: arrow_query.py <database file> <table>", file=sys.stderr)
        return 1

    path, table_name = argv[1], argv[2]
    with sa17.Database.open(path) as db:
        table = db.table(table_name)
        schema = table.arrow_schema()
        print("schema:", ", ".join(f"{n}:{t}" for n, t in zip(schema.names, schema.types, strict=True)))

        arrow_table = table.to_arrow()
        print(f"materialized rows={arrow_table.num_rows} cols={arrow_table.num_columns}")

        stream = table.batches(batch_size=8192)
        conn = duckdb.connect()
        conn.register("source", stream)
        rows = conn.sql("SELECT COUNT(*) FROM source").fetchall()
        print(f"duckdb count={rows[0][0]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
