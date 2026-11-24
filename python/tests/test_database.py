"""Tests for opening databases, catalog metadata, keys, errors, and raw pages."""

from __future__ import annotations

import pytest

import sa17

from .conftest import Fixtures


def test_open_and_metadata(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        assert db.superblock.version == "3.201.12"
        assert db.superblock.database_page_count > 0
        assert db.text_encoding == sa17.TextEncoding.Windows1252
        assert db.identity.page_count > 0
        assert db.key.sign == sa17.KeySign.Positive
        assert db.catalog_problems == []
        assert len(db) >= 4
        assert "plants" in db
        assert "PLANTS" in db
        assert "gardener.plants" in db
        assert "missing_table" not in db
        assert "Database(" in repr(db)


def test_from_bytes_and_explicit_negative_key(fixtures: Fixtures) -> None:
    raw = fixtures.negative_key.read_bytes()
    with sa17.Database.from_bytes(raw) as db:
        assert db.key.sign == sa17.KeySign.Negative
        recovered_key = db.key

    options = sa17.OpenOptions(key=recovered_key)
    with sa17.Database.from_bytes(raw, options=options) as db:
        assert db.key == recovered_key
        items = db["items"]
        assert items.row_count == 60

    wrong_key = sa17.StoreKey(
        mask=recovered_key.mask,
        addend=recovered_key.addend + 1,
        sign=recovered_key.sign,
    )
    with pytest.raises(sa17.Sa17Error) as caught:
        sa17.Database.from_bytes(raw, options=sa17.OpenOptions(key=wrong_key))
    assert caught.value.code == "wrong_key"


def test_utf8_database(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.utf8)) as db:
        assert db.text_encoding == sa17.TextEncoding.Utf8
        words = [row["word"] for row in db["words"].rows()]
        assert words == ["日本語", "résumé"]


def test_schema_keys_indexes_and_views(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        plants = db.table("plants")
        assert plants.owner == "gardener"
        assert plants.kind == sa17.TableKind.Base
        assert not plants.is_system
        assert plants.stores_rows
        assert plants.primary_key == ["id"]

        assert len(plants.foreign_keys) == 1
        fk = plants.foreign_keys[0]
        assert fk.name == "fk_plants_bed"
        assert fk.columns == ["bed_id"]
        assert fk.target_table == "beds"
        assert fk.target_columns == ["id"]

        target = plants.target("fk_plants_bed")
        assert target is not None
        assert target.name == "beds"
        assert plants.target("no_such_fk") is None

        index_names = {idx.name for idx in plants.indexes}
        assert "idx_plants_name" in index_names

        info = plants.info
        assert info.schema_version == 1
        assert len(info.pages) >= 1
        assert len(info.extension_pages) >= 1

        summary = db.table("plant_summary")
        assert summary.kind == sa17.TableKind.View
        assert not summary.stores_rows

        by_id = db.table_by_id(plants.id)
        assert by_id is not None
        assert by_id.name == "plants"
        assert db.table_by_id(999_999) is None


def test_lookup_and_format_errors(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        with pytest.raises(sa17.Sa17Error) as caught:
            db.table("nonexistent")
        assert caught.value.code == "unknown_table"

        with pytest.raises(KeyError):
            _ = db["nonexistent"]

    with pytest.raises(sa17.Sa17Error) as caught:
        sa17.Database.open("/nonexistent/path/database.db")
    assert caught.value.code == "io"

    with pytest.raises(sa17.Sa17Error) as caught:
        sa17.Database.from_bytes(b"not a database")
    assert caught.value.code == "format"


def test_page_store_and_raw_pages(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        store = db.store()
        assert store.page_size == 4096
        assert store.size == store.page_count * 4096
        assert store.check(0) == sa17.PageState.Written
        assert store.check(10) == sa17.PageState.Written
        assert len(store.stored(10)) == 4096

        page10 = store.page(10)
        assert page10.number == 10
        assert page10.stored_number == 10
        assert page10.kind == sa17.PageKind.PageList
        assert len(page10.data()) == 4096

        plants_page_num = db["plants"].info.pages[0]
        data_page = store.page(plants_page_num)
        assert data_page.kind == sa17.PageKind.Table
        assert data_page.table_id == db["plants"].id
        records = data_page.records()
        assert len(records) >= 3
        first_bytes = data_page.record_bytes(records[0].address.slot)
        assert first_bytes is not None
        assert len(first_bytes) == records[0].length

    with sa17.PageStore.open(str(fixtures.garden)) as standalone:
        assert standalone.superblock.version == "3.201.12"
        assert "PageStore(" in repr(standalone)
