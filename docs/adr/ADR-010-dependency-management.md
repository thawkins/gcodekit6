# ADR-010: Dependency Management Strategy

## Status
Accepted

Amended 2026-11-28: dropped the third package ecosystem (no `package.json` exists) and the GTK dependency-count claim; `cargo audit` is a manual monthly step, not CI; unused dependencies are caught by `cargo machete` in CI; recorded the real `cargo deny` licenses, bans and sources settings.

## Date
2026-02-18

## Context
The project has hundreds of transitive dependencies; count them with `grep -c '^\[\[package\]\]' Cargo.lock`. Unmanaged dependencies lead to security vulnerabilities, license compliance issues, duplicate crate versions increasing compile times, and breaking changes on update.

## Decision
Implement a multi-layered dependency management strategy:

### Automated Updates
- **Dependabot** configured for weekly PRs (Cargo, GitHub Actions)
- Patch and minor updates grouped to reduce PR noise
- CI must pass before merging any dependency update

### Auditing
- **`cargo deny check`** runs in the Code Quality workflow on every push and pull request to `master`, `develop` and `devel`:
  - **Advisories**: Deny known vulnerabilities (RustSec database)
  - **Licenses**: an allowlist in `deny.toml` of permissive licenses (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib and others) plus the weak-copyleft MPL-2.0 and EUPL-1.2, with per-crate exceptions for two crates
  - **Bans**: Duplicate crate versions are allowed (`multiple-versions = "allow"`)
  - **Sources**: crates.io, plus an allowlist of git repositories (craft-libs and its egui and winit forks, core2)
- **`cargo audit`** runs by hand as part of `scripts/monthly-dependency-review.sh`; it is not in CI
- **`cargo machete`** runs in the Code Quality workflow and fails when a manifest declares a dependency its crate never uses

### Workspace Dependencies
- Common dependencies declared in workspace `[workspace.dependencies]`
- Sub-crates reference via `{ workspace = true }`
- Ensures version consistency across all workspace members

### Update Policy
- **Patch updates**: Apply automatically via Dependabot PRs
- **Minor updates**: Review changelog, apply if backward-compatible
- **Major updates**: Plan migration, test thoroughly, document in CHANGELOG
- **Security fixes**: Apply immediately regardless of version bump

### Configuration Files
- `deny.toml` — cargo-deny rules (advisories, licenses, bans, sources)
- `.cargo/mutants.toml` — mutation testing configuration
- `.github/dependabot.yml` — automated update schedule

## Consequences

**Positive:**
- Security vulnerabilities caught early via automated scanning
- License compliance verified on every CI run
- Unused dependencies don't accumulate
- Consistent versions across workspace crates

**Negative:**
- Weekly Dependabot PRs require review attention
- `cargo deny` may require allowlist updates when adding new dependencies
- Some transitive duplicates are unavoidable (e.g., bitflags v1 + v2)

## Alternatives Considered
- **No automated scanning**: Less maintenance but security risks go unnoticed
- **Renovate instead of Dependabot**: More configurable but Dependabot is native to GitHub
- **Vendoring dependencies**: Maximum reproducibility but enormous repo size and update burden
