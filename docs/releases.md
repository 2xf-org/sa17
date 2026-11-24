# Releases

The `sa17` crate and the `sa17` Python package release together at one version, which `Cargo.toml` and `python/pyproject.toml` both name.

## Versions

A version is the date of its release, as `YYYYMM.DD.N`: the year and month, the day, and the count of that day's releases, from 0. The first release on 24 November 2026 is `202611.24.0`, and a second that day is `202611.24.1`. Cargo allows no leading zero, so 5 January 2027 is `202701.5.0`.

Cargo reads the first number as a major version, so it takes the releases of one month for compatible and those of two months for not. Neither is a promise, so applications name one exact version.

## Before the tag

1. Change `version` in the root `Cargo.toml` to the date of the release, the `sa17 = { version = "=YYYYMM.DD.N" }` requirement under `[workspace.dependencies]` in the same file, and `version` in `python/pyproject.toml`.
2. Change the version in the `sa17 = "=YYYYMM.DD.N"` samples in `README.md`, `rust/sa17/README.md` and `docs/rust/README.md`.
3. Update `Cargo.lock` and `python/uv.lock`, run `bimetal build --config rust/sa17-py`, and commit.
4. Wait for CI to pass on that commit on `main`.

## The tag

Tag the commit `vYYYYMM.DD.N`, such as `v202611.24.0`, and push the tag. `.github/workflows/release.yml` then runs these jobs:

| Job | What it does |
| --- | --- |
| Verify release version | Fails unless the tag, the manifests, the requirement and the samples agree |
| One wheel per platform | Builds and tests the wheel for Linux x86_64 and arm64, macOS arm64 and x86_64, and Windows x86_64 |
| Publish to crates.io | Publishes `sa17`, with the `CARGO_REGISTRY_TOKEN` secret of the `release` environment |
| Publish to PyPI | Publishes the wheels through PyPI's trusted publisher, so there is no token |
| Create GitHub Release | Creates the release, named after the tag, with the wheels, their `SHA256SUMS` and how to install the version |

Both registries keep a version for ever, so publishing comes after every build. The registries publish in separate jobs, so when one fails, run that job again.
