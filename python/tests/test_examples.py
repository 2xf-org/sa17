"""Tests that every runnable Python example executes cleanly on synthetic fixtures."""

from __future__ import annotations

import pathlib
import subprocess
import sys

from .conftest import Fixtures

EXAMPLES_DIR = pathlib.Path(__file__).resolve().parents[1] / "examples"


def run_example(name: str, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(EXAMPLES_DIR / f"{name}.py"), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def test_tables_example(fixtures: Fixtures) -> None:
    result = run_example("tables", str(fixtures.garden))
    assert result.returncode == 0, result.stderr
    assert "gardener.plants" in result.stdout
    assert "FK fk_plants_bed" in result.stdout


def test_rows_example(fixtures: Fixtures) -> None:
    result = run_example("rows", str(fixtures.garden), "plants", "id", "name")
    assert result.returncode == 0, result.stderr
    assert "'name': 'Fern'" in result.stdout
    assert "'name': 'Bamboo'" in result.stdout
    assert "'name': 'Moss'" in result.stdout


def test_arrow_query_example(fixtures: Fixtures) -> None:
    result = run_example("arrow_query", str(fixtures.garden), "plants")
    assert result.returncode == 0, result.stderr
    assert "materialized rows=3 cols=6" in result.stdout
    assert "duckdb count=3" in result.stdout


def test_verify_example(fixtures: Fixtures) -> None:
    sound = run_example("verify", str(fixtures.garden))
    assert sound.returncode == 0, sound.stderr
    assert "sound=True" in sound.stdout
    assert "page10=page_list" in sound.stdout

    damaged = run_example("verify", str(fixtures.damaged))
    assert damaged.returncode == 1
    assert "sound=False" in damaged.stdout
