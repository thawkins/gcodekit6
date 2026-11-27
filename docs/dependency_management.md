# Dependency Management Guide

This document describes the dependency management process for GCodeKit6.

## Automated Updates (Dependabot)

Dependabot is configured to automatically create PRs for dependency updates:

- **Cargo dependencies**: Weekly on Mondays at 09:00 UTC
- **GitHub Actions**: Weekly on Mondays at 09:00 UTC

Minor and patch updates are grouped together to reduce PR noise. The egui family
(`egui`, `eframe`, `egui_*`, `egui-*`, `epaint`, `emath`, `ecolor`) is grouped
separately.

Dependabot ignores two upgrades (`ignore` in `.github/dependabot.yml`):

- egui family 0.38 and later: egui is bumped together with the craft-libs pin
  ([ADR-011](adr/ADR-011-egui-craft-libs-pin.md)).
- `nalgebra` 0.34 and later: `csgrs` still depends on nalgebra 0.33.

## CI Dependency Check

The `dependency-audit` job in `.github/workflows/code-quality.yml` runs
`cargo deny check` on every push and pull request to `master`, `develop` and
`devel`. Its policy is in `deny.toml`.

### Reviewing Dependabot PRs

1. **Minor/Patch updates**: Generally safe to merge after CI passes
2. **Major updates**: Review changelog for breaking changes before merging
3. **Security updates**: Prioritize and merge promptly after CI passes

## Monthly Manual Review

In addition to automated updates, perform a monthly manual review to:

- Check for outdated dependencies that Dependabot may have missed
- Review security advisories
- Evaluate major version updates
- Clean up unused dependencies

### Prerequisites

Install the required cargo tools:

```bash
cargo install cargo-outdated cargo-audit
```

### Running the Review

Use the provided script:

```bash
./scripts/monthly-dependency-review.sh
```

This will:
1. Check for outdated dependencies with `cargo outdated`
2. Run security audit with `cargo audit`
3. Check for duplicate dependencies
4. Generate a report in `target/temp/dependency-review-YYYY-MM.md`

### Applying Updates

To apply safe (minor/patch) updates:

```bash
./scripts/monthly-dependency-review.sh --update
```

For major version updates, update manually:

```bash
# Edit Cargo.toml to update version
cargo update -p <package>
cargo nextest run --workspace
```

## Security Vulnerability Response

When security vulnerabilities are discovered:

1. **Critical/High severity**: Fix within 24 hours
2. **Medium severity**: Fix within 1 week
3. **Low severity**: Fix in next monthly review

### Checking for Vulnerabilities

```bash
cargo audit
```

### Fixing Vulnerabilities

1. Update the affected dependency if a fix is available
2. If no fix available, evaluate:
   - Can we replace the dependency?
   - Can we work around the vulnerable code path?
   - Document the risk if we must accept it temporarily

## Dependency Guidelines

### Adding New Dependencies

Before adding a new dependency:

1. **Evaluate necessity**: Can we implement this ourselves reasonably?
2. **Check maintenance status**: Is the crate actively maintained?
3. **Review security**: Run `cargo audit` after adding
4. **Check license compatibility**: Ensure license is compatible (MIT, Apache 2.0, BSD)
5. **Consider size**: Avoid heavy dependencies for simple features

### Preferred Crates

These are the crates the workspace already uses for each job. Reuse them before
adding another.

| Category | Crate(s) |
|----------|----------|
| Error handling | `thiserror`, `anyhow` |
| Serialization | `serde`, `serde_json` |
| Logging | `tracing` |
| Date/time | `chrono` |
| Testing | Built-in `#[test]`, `proptest` for property testing |

### Version Constraints

Use appropriate version constraints in `Cargo.toml`:

```toml
# Prefer: Compatible updates (most common)
serde = "1.0"

# When needed: Exact version (rare, avoid if possible)
some-crate = "=1.2.3"

# For development dependencies: More relaxed
[dev-dependencies]
proptest = "1"
```

## Duplicate Dependencies

Some duplicate versions are unavoidable because of transitive dependencies. To
list the current duplicates:

```bash
cargo tree --duplicates
```

## Related Documentation

- [CONTRIBUTING.md](../CONTRIBUTING.md) - Contribution guidelines
