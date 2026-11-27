## Description

<!-- Provide a brief description of the changes in this PR -->

## Related Issues

<!-- Link to any related issues using #issue_number -->

Fixes #

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Refactoring (no functional changes)
- [ ] Documentation update
- [ ] Build/CI changes

## PR Checklist

<!-- Complete this checklist before requesting review -->

- [ ] No `.unwrap()` calls (CI clippy runs `-W clippy::unwrap_used -D warnings` on every target and fails on any; use `.expect("why")` in tests, `?` or handling in production)
- [ ] `cargo fmt` passed
- [ ] `cargo clippy` has no new warnings
- [ ] Tests added for new functionality
- [ ] `cargo nextest run --workspace` and `cargo test --workspace --doc` pass
- [ ] kittest snapshots updated if UI changed
- [ ] Public APIs documented with `///`
- [ ] Error cases handled (no silent failures)
- [ ] No debug `eprintln!()` or `println!()` statements
- [ ] Changelog entry added

## Testing Instructions

<!-- How can reviewers test these changes? -->

1. 
2. 
3. 

## Screenshots (if UI change)

<!-- Add screenshots for any UI changes -->

## Additional Notes

<!-- Any additional context or notes for reviewers -->

