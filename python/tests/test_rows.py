"""Tests for streaming row iteration, projection, random lookup, and typed accessors."""

from __future__ import annotations

import datetime
from decimal import Decimal

import pytest

import sa17

from .conftest import Fixtures


def test_row_streaming_schema_evolution_and_long_values(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        plants = db["plants"]
        rows = list(plants.rows())
        assert len(rows) == 3

        # Row 0: schema version 0 gets absent value "legacy" for column `notes`.
        r0 = rows[0]
        assert r0.address is not None
        assert "name" in r0
        assert "missing" not in r0
        assert len(r0) == 6
        assert r0["id"] == 1
        assert r0["name"] == "Fern"
        assert r0["bed_id"] == 1
        assert r0["height"] == pytest.approx(0.45)
        assert r0.get_decimal("price") == "12.50"
        assert Decimal(r0.get_decimal("price") or "0") == Decimal("12.50")
        assert r0["notes"] == "legacy"

        # Row 1: forwarded row with compressed long text value.
        r1 = rows[1]
        assert r1["id"] == 2
        assert r1["name"] == "Bamboo"
        assert r1["notes"] == "Tall perennial evergreen grass. " * 40

        # Row 2: deleted row (id=99) skipped; negative decimal and NULLs preserved.
        r2 = rows[2]
        assert r2["id"] == 3
        assert r2["name"] == "Moss"
        assert r2["bed_id"] is None
        assert r2["height"] is None
        assert r2.get_decimal("price") == "-0.50"
        assert r2["notes"] is None


def test_column_projection_and_random_access(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        plants = db["plants"]

        projected = [row.to_dict() for row in plants.scan(["name", "id"])]
        assert projected == [
            {"id": 1, "name": "Fern"},
            {"id": 2, "name": "Bamboo"},
            {"id": 3, "name": "Moss"},
        ]

        addresses = list(plants.row_addresses())
        assert len(addresses) == 3

        second = plants.row(addresses[1], columns=["name", "price"])
        assert second is not None
        assert second.columns == ["name", "price"]
        assert second["name"] == "Bamboo"
        assert second["price"] == "29.99"

        with pytest.raises(sa17.Sa17Error) as caught:
            plants.scan(["does_not_exist"])
        assert caught.value.code == "unknown_column"

        with pytest.raises(sa17.Sa17Error) as caught:
            _ = second["id"]
        assert caught.value.code == "unknown_column"


def test_all_types_and_typed_row_accessors(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.types)) as db:
        rows = list(db["samples"].rows())
        assert len(rows) == 2

        first, nulls = rows
        assert first["c_bit"] is True
        assert first["c_tinyint"] == 200
        assert first["c_smallint"] == -32000
        assert first["c_usmallint"] == 65000
        assert first["c_int"] == -1_000_000
        assert first["c_uint"] == 3_000_000_000
        assert first["c_bigint"] == -9_000_000_000_000
        assert first["c_ubigint"] == 18_000_000_000_000_000_000
        assert first["c_float"] == pytest.approx(1.5)
        assert first["c_double"] == pytest.approx(1234.567_89)
        assert first.get_decimal("c_numeric") == "123456.789"
        assert first["c_char"] == "abc"
        assert first["c_varchar"] == "café"
        assert first["c_longvarchar"] == "Long chained narrative. " * 60
        assert first.get_bytes("c_binary") == b"binary-blob-\x00\xff\x10\x20"
        assert first.get_bytes("c_varbinary") == b"\xde\xad\xbe\xef"
        assert first.get_bytes("c_longbinary") == bytes(i % 251 for i in range(1200))
        assert first.get_date("c_date") == datetime.date(2024, 1, 1)
        assert first["c_time"] == "14:30:45.123456"
        assert first["c_timestamp"] == "2024-01-01 14:30:45.654321"

        assert all(value is None for value in nulls.values())
        assert nulls.get_bytes("c_binary") is None
        assert nulls.get_date("c_date") is None
        assert nulls.get_decimal("c_numeric") is None


def test_longest_value_limit_enforced(fixtures: Fixtures) -> None:
    options = sa17.OpenOptions(longest_value=100)
    with sa17.Database.open(str(fixtures.garden), options=options) as db:
        plants = db["plants"]
        # Scanning without the compressed `notes` column succeeds.
        names = [row["name"] for row in plants.scan(["name"])]
        assert names == ["Fern", "Bamboo", "Moss"]

        # Reading `notes` fails on Bamboo's compressed value (> 100 bytes).
        cursor = plants.rows()
        assert next(cursor)["name"] == "Fern"
        with pytest.raises(sa17.Sa17Error) as caught:
            next(cursor)
        assert caught.value.code == "row"
