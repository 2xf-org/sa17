"""Inspect tables, columns, keys, and indexes in a SQL Anywhere 17 database.

Usage:
    python examples/tables.py database.db
"""

from __future__ import annotations

import sys

import sa17


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: tables.py <database file>", file=sys.stderr)
        return 1

    with sa17.Database.open(argv[1]) as db:
        print(
            f"version={db.superblock.version} "
            f"encoding={db.text_encoding.value} "
            f"pages={db.identity.page_count}"
        )
        for table in db.tables():
            if table.is_system:
                continue
            cols = ", ".join(f"{c.name}:{c.sql_type}" for c in table.columns)
            print(f"{table.row_count:>10}  {table.owner}.{table.name} ({cols})")
            if table.primary_key:
                print(f"            PK ({', '.join(table.primary_key)})")
            for fk in table.foreign_keys:
                target = fk.target_table or f"#{fk.target_id}"
                print(
                    f"            FK {fk.name}: "
                    f"({', '.join(fk.columns)}) -> {target}({', '.join(fk.target_columns)})"
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
