"""Stream rows of a table, optionally projecting selected columns.

Usage:
    python examples/rows.py database.db plants [column ...]
"""

from __future__ import annotations

import sys

import sa17


def main(argv: list[str]) -> int:
    if len(argv) < 3:
        print("usage: rows.py <database file> <table> [column ...]", file=sys.stderr)
        return 1

    path, table_name, *columns = argv[1:]
    with sa17.Database.open(path) as db:
        table = db.table(table_name)
        cursor = table.scan(columns) if columns else table.rows()
        for row in cursor:
            print(row.to_dict())
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
