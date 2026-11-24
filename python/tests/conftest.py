"""Shared synthetic SA17 database fixtures for Python integration tests."""

from __future__ import annotations

import pathlib
import subprocess
from dataclasses import dataclass

import pytest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class Fixtures:
    """Paths to synthetic SA17 database files generated for the test session."""

    garden: pathlib.Path
    types: pathlib.Path
    negative_key: pathlib.Path
    utf8: pathlib.Path
    damaged: pathlib.Path


@pytest.fixture(scope="session")
def fixtures(tmp_path_factory: pytest.TempPathFactory) -> Fixtures:
    """Generate fresh synthetic SA17 databases once per test session."""
    output_dir = tmp_path_factory.mktemp("sa17-fixtures")
    subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "sa17-synthetic",
            "--example",
            "fixtures",
            "--",
            str(output_dir),
        ],
        cwd=REPO_ROOT,
        check=True,
    )
    return Fixtures(
        garden=output_dir / "garden.db",
        types=output_dir / "types.db",
        negative_key=output_dir / "negative_key.db",
        utf8=output_dir / "utf8.db",
        damaged=output_dir / "damaged.db",
    )
