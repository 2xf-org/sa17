"""Tests for zero-copy Apache Arrow export, PyArrow, Polars, and DuckDB."""

from __future__ import annotations

import datetime
from decimal import Decimal

import duckdb
import polars as pl
import pyarrow as pa
import pytest

import sa17

from .conftest import Fixtures


def test_arrow_schema_and_all_types_table(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.types)) as db:
        samples = db["samples"]
        schema = samples.arrow_schema()
        assert isinstance(schema, pa.Schema)
        types = dict(zip(schema.names, schema.types, strict=True))
        assert types == {
            "c_bit": pa.bool_(),
            "c_tinyint": pa.uint8(),
            "c_smallint": pa.int16(),
            "c_usmallint": pa.uint16(),
            "c_int": pa.int32(),
            "c_uint": pa.uint32(),
            "c_bigint": pa.int64(),
            "c_ubigint": pa.uint64(),
            "c_float": pa.float32(),
            "c_double": pa.float64(),
            "c_numeric": pa.decimal128(15, 3),
            "c_char": pa.string(),
            "c_varchar": pa.string(),
            "c_longvarchar": pa.large_string(),
            "c_binary": pa.binary(),
            "c_varbinary": pa.binary(),
            "c_longbinary": pa.large_binary(),
            "c_date": pa.date32(),
            "c_time": pa.time64("us"),
            "c_timestamp": pa.timestamp("us"),
        }

        table = samples.to_arrow()
        assert isinstance(table, pa.Table)
        assert table.num_rows == 2

        rows = table.to_pylist()
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
        assert first["c_double"] == pytest.approx(1_234.567_89)
        assert first["c_numeric"] == Decimal("123456.789")
        assert first["c_char"] == "abc"
        assert first["c_varchar"] == "café"
        assert first["c_longvarchar"] == "Long chained narrative. " * 60
        assert first["c_binary"] == b"binary-blob-\x00\xff\x10\x20"
        assert first["c_varbinary"] == b"\xde\xad\xbe\xef"
        assert first["c_longbinary"] == bytes(i % 251 for i in range(1200))
        assert first["c_date"] == datetime.date(2024, 1, 1)
        assert first["c_time"] == datetime.time(14, 30, 45, 123_456)
        assert first["c_timestamp"] == datetime.datetime(2024, 1, 1, 14, 30, 45, 654_321)

        assert all(value is None for value in nulls.values())


def test_empty_table_preserves_arrow_schema(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        empty = db["empty_table"]
        table = empty.to_arrow()
        assert table.num_rows == 0
        assert dict(zip(table.schema.names, table.schema.types, strict=True)) == {
            "id": pa.int32(),
            "title": pa.string(),
        }


def test_streaming_batches_polars_and_duckdb(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        plants = db["plants"]

        # Stream in 2-row batches so 3 rows produce two batches (2 + 1).
        reader = plants.batches(columns=["id", "name", "price"], batch_size=2)
        assert isinstance(reader, pa.RecordBatchReader)
        batches = list(reader)
        assert [batch.num_rows for batch in batches] == [2, 1]

        # Polars consumes the Arrow table directly without copying.
        frame = pl.from_arrow(plants.to_arrow(["id", "name", "height", "price"]))  # pyright: ignore[reportUnknownMemberType]
        assert isinstance(frame, pl.DataFrame)
        assert frame.shape == (3, 4)
        tall = frame.filter(pl.col("height") > 1.0)["name"].to_list()  # pyright: ignore[reportUnknownMemberType]
        assert tall == ["Bamboo"]

        # DuckDB queries a lazy RecordBatchReader directly over the C stream interface.
        stream = plants.batches(columns=["id", "name", "price"], batch_size=2)
        conn = duckdb.connect()
        conn.register("stream", stream)
        result = conn.sql(
            "SELECT name, price FROM stream WHERE price > 0 ORDER BY price DESC"
        ).fetchall()
        assert result == [
            ("Bamboo", Decimal("29.99")),
            ("Fern", Decimal("12.50")),
        ]


def test_zero_batch_size_rejected(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        with pytest.raises(sa17.Sa17Error) as caught:
            db["plants"].batches(batch_size=0)
        assert caught.value.code == "arrow"

        with pytest.raises(sa17.Sa17Error) as caught:
            db["plants"].to_arrow(batch_size=0)
        assert caught.value.code == "arrow"
