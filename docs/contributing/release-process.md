# Release Process

This page documents how SlateDuck releases are built, tested, tagged, and published. It is primarily for maintainers but is documented publicly for transparency.

## Version Numbering

SlateDuck follows semantic versioning (SemVer):

- **Major (X.0.0):** Breaking changes to the catalog format or public API
- **Minor (0.X.0):** New features, backward-compatible format changes
- **Patch (0.0.X):** Bug fixes only, no format or API changes

During the 0.x series (pre-1.0), minor versions may include breaking changes with advance notice in the changelog.

## Release Steps

### 1. Prepare the Release

- Update version numbers in all `Cargo.toml` files (workspace and per-crate)
- Update the changelog with all changes since the last release
- Run the full test suite: `cargo test`
- Run benchmarks and compare to the previous release
- Build documentation: `mkdocs build --strict`

### 2. Create the Release PR

- Open a PR titled `release: v0.X.Y`
- The PR should contain only version bumps and changelog updates
- CI runs the full test matrix (multiple OS, multiple Rust versions)
- Require at least one reviewer approval

### 3. Tag and Publish

After the PR is merged:

```bash
git tag v0.X.Y
git push origin v0.X.Y
```

The tag triggers the release CI workflow which:
- Builds binaries for all supported platforms (Linux x86_64, macOS ARM64, Windows x86_64)
- Creates a GitHub Release with the binaries attached
- Publishes the Docker image to GitHub Container Registry
- Deploys the updated documentation to GitHub Pages

### 4. Post-Release

- Announce the release (GitHub Discussions, relevant community channels)
- Monitor for regressions in the first 24-48 hours
- Begin the next development cycle (bump version to next `-dev` suffix)

## Hotfix Process

For critical bug fixes that cannot wait for the next regular release:

1. Branch from the release tag: `git checkout -b hotfix/v0.X.Y+1 v0.X.Y`
2. Apply the fix and add a regression test
3. Run the full test suite
4. Merge to main and tag the new patch version

## Supported Versions

Only the latest minor version receives patches. Older minor versions are unsupported. Users are expected to track the latest version.
