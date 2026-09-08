---
name: gate
description: Run every Meteor quality gate (ESLint, tsc, Vitest, clippy -D warnings, cargo test, i18n parity) and report the real output of each. Use before saying a change is done, before a release, or when the user says "gates", "check", "¿está verde?".
---

# Quality gates

Run all of them, in this order, and report **actual output** — never a summary
you inferred. If a gate cannot run, say so and why (that is a valid result; a
silent skip is not).

## Steps

1. **Prerequisite check.** `cargo` needs the bundled resources declared in
   `tauri.conf.json`:
   ```bash
   ls src-tauri/binaries/*.exe
   ```
   If they are missing, run `powershell -File scripts/fetch-binaries.ps1`
   (downloads PresentMon with a SHA-256 check, publishes the .NET sidecar).
   Never fake the binaries.

2. **Frontend**, from the repo root:
   ```bash
   npm run lint          # eslint, 0 errors required (warnings are the design drift backlog)
   npx tsc --noEmit      # must be clean
   npm run test          # vitest: fuzzy search + i18n catalog parity
   npm run build         # next build, catches export-time failures
   ```

3. **Rust**, from `src-tauri/`:
   ```bash
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```

4. `npm run check` runs lint + tsc + vitest + clippy + cargo test in one command;
   use it when you only need pass/fail, and the individual commands when you need
   to show output per gate.

## Reporting

Use the format from `CLAUDE.md` §13:

| Gate | Result | Notes |
|---|---|---|
| `eslint` | PASS / FAIL | N errors, N warnings |
| `tsc --noEmit` | PASS / FAIL | first errors verbatim |
| `vitest` | PASS / FAIL | N passed |
| `next build` | PASS / FAIL | |
| `cargo clippy -D warnings` | PASS / FAIL | first errors verbatim |
| `cargo test` | PASS / FAIL | N passed |

Rules:
- Quote the first ~20 lines of any failure verbatim; do not paraphrase an error.
- A gate that could not run is reported as **NOT RUN** with the reason, never as
  a pass.
- Do not fix anything while reporting unless the user asked for fixes; list what
  is broken first.
