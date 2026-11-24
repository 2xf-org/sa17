"""Verify database integrity and inspect the page store.

Usage:
    python examples/verify.py database.db
"""

from __future__ import annotations

import sys

import sa17


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: verify.py <database file>", file=sys.stderr)
        return 1

    with sa17.Database.open(argv[1]) as db:
        store = db.store()
        page10 = store.page(10)
        report = db.verify()
        print(
            f"sound={report.is_sound} pages={report.pages} "
            f"tables={report.tables} rows={report.rows} "
            f"page10={page10.kind.value}"
        )
        for problem in report.problems:
            print(f"{problem.kind.value}: {problem.message}", file=sys.stderr)
        return 0 if report.is_sound else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
