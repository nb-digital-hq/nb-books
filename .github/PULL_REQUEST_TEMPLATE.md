## Problem

<!-- Link the issue or the ADR. A pull request without context cannot be reviewed properly. -->

Closes #

## Approach

<!-- One or two sentences about the approach, not a summary of the diff. -->

## How to verify

<!-- Manual steps, or the name of the added test. Another person must be able to check it. -->

---

## Definition of done

See [`AGENTS.md`](../blob/main/AGENTS.md), section "Verification".

- [ ] The acceptance criterion is met and can be checked by another person
- [ ] There are tests for the added logic, and one regression test per fixed defect
- [ ] `just verify` (or `pnpm verify` plus the Rust checks) passes
- [ ] If the change crosses the IPC bridge, the generated bindings are up to date
- [ ] If it has architectural or user-facing impact, `docs/` was updated in this same pull request
- [ ] If it affects the interface, full keyboard navigation was verified
- [ ] If it introduces a component with a health state, its chaos-suite scenario exists
- [ ] Commits are signed off with `git commit -s` (DCO)

## Impact

- [ ] Adds or changes dependencies <!-- justify per docs/architecture/dependency-policy.md -->
- [ ] Changes the database schema <!-- include a migration and a migration test -->
- [ ] Adds or changes IPC commands <!-- justify the larger attack surface -->
- [ ] Affects personal data or anything that leaves the device
- [ ] None of the above
