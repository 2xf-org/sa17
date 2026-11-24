"""Tests for full-database integrity verification."""

from __future__ import annotations

import sa17

from .conftest import Fixtures


def test_sound_database_verifies_clean(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.garden)) as db:
        report = db.verify()
        assert report.is_sound
        assert report.catalog_problems == 0
        assert report.damaged_pages == 0
        assert report.unreadable_rows == 0
        assert report.unreadable_pages == 0
        assert report.differing_tables == 0
        assert report.problems == []


def test_damaged_database_reports_problems(fixtures: Fixtures) -> None:
    with sa17.Database.open(str(fixtures.damaged)) as db:
        report = db.verify()
        assert not report.is_sound
        assert report.damaged_pages >= 1
        assert report.unreadable_pages >= 1
        assert len(report.problems) >= 1
        kinds = {problem.kind for problem in report.problems}
        assert sa17.VerificationProblemKind.Page in kinds
