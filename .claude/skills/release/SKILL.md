---
name: release
description: Bump Meteor's version consistently across package.json, package-lock.json, Cargo.toml, Cargo.lock and tauri.conf.json, update CHANGELOG.md, and prepare the `rel:` commit + `vX.Y.Z` tag that triggers the GitHub release workflow. Use when the user says "release", "bump version", "nueva versión", "publicar", or "/release <version>".
---

# Release Meteor

Argument: the new semver (e.g. `0.1.2`). If missing, ask for it or propose the next patch.

## Steps

1. **Preflight** (stop and report if any fails):
   - `git status --porcelain` must be clean apart from the files this skill edits.
   - `npx tsc --noEmit` passes.
   - `cd src-tauri && cargo check` passes (needs `binaries/PresentMon.exe` and `binaries/cputemp.exe` present locally; if absent, say so and continue with the typecheck only).
2. **Bump the version in all five files** (they must match exactly):
   - `package.json` → `"version"`
   - `package-lock.json` → top-level `"version"` **and** `packages[""].version` (this file is the one that gets forgotten)
   - `src-tauri/Cargo.toml` → `[package] version`
   - `src-tauri/Cargo.lock` → the `[[package]] name = "meteor"` entry's `version`
   - `src-tauri/tauri.conf.json` → `"version"`
   Verify with: `grep -n '"version"' package.json src-tauri/tauri.conf.json; grep -n '^version' src-tauri/Cargo.toml; grep -n -A1 'name = "meteor"' src-tauri/Cargo.lock`
3. **CHANGELOG.md**: rename the `## [No publicado]` section to `## [X.Y.Z] — YYYY-MM-DD` (today) and add a fresh empty `## [No publicado] — Trabajo en curso` above it. Keep the Keep-a-Changelog subsections (Añadido / Cambiado / Corregido).
4. Show the diff and **ask the user to confirm** before committing.
5. Commit with message `rel: update version to X.Y.Z in package.json, Cargo.toml, Cargo.lock, and tauri.conf.json` (matches the existing history) followed by the required Co-Authored-By / Claude-Session trailers.
6. Tag: `git tag vX.Y.Z`. Do **not** push unless the user explicitly asks; pushing the tag triggers `.github/workflows/release.yml`, which builds, signs (updater keypair from repo secrets) and publishes the release plus `latest.json`.

## Notes

- Updater consumers read `https://github.com/MrRobot4042212/Meteor/releases/latest/download/latest.json`; a bad tag is user-visible within minutes, so never push a tag on a red typecheck.
- Version history was reset once (1.0.x → 0.0.x); keep monotonic semver from now on.
