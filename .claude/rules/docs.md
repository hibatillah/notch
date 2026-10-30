---
paths:
  - "docs/**"
  - "README.md"
---

# Docs

- State the current behavior and where it lives; leave out how it used to work and what a change replaced.
- Keep each fact in one doc and link to it from the others: settings keys and defaults in `docs/configuration.md`, limits
  and gates in `docs/testing.md`, measured numbers in `docs/performance.md`.
- `docs/performance.md` holds the latest baseline only, with its date, machine, workload and the command that produced
  it. Replace the numbers on each run; leave failed or superseded runs to git history.
- Write the user-facing default of a setting in the `README.md` table and in `docs/configuration.md` together.
