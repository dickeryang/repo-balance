# Contributing

[简体中文](CONTRIBUTING.md) | English

Thanks for your interest in RepoBalance. This project is a **fully local** Git repository health checker: **no network requests, no data collection whatsoever** — every change must respect that boundary.

## 1. Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.88+ | Dependency-graph MSRV floor, see `rust-version` in the root `Cargo.toml` |
| Node.js | 14.18+ | Minimum for Vite 4 |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |

## 2. Development workflow

```bash
# 1. Install frontend dependencies (npm must point at the official registry)
cd ui && npm install --registry https://registry.npmjs.org/ && cd ..

# 2. All-in-one dev (starts the Vite dev server plus Rust hot reload)
cargo tauri dev
```

All quality gates must pass before you push:

```bash
cargo fmt --all -- --check               # formatting (rustfmt.toml: max_width = 100)
cargo clippy --all-targets --workspace -- -D warnings   # lint — warnings are errors in CI
cargo test --workspace                   # unit tests (real git repos via git2 fixtures)
cd ui && npm run lint && npm run build   # frontend ESLint + build
```

These are exactly the checks CI runs; a red check blocks the merge.

## 3. Code discipline

1. **Never cross the layer boundary**: `rb-core` (the checker core) **must not** contain `tauri` / `std::fs`; `rb-app` (the shell) must not contain `git2`. All file reading goes through the `git` module, and the frontend never touches the filesystem.
2. **No `unwrap()` / `expect()` outside tests**: propagate errors with `?`.
3. **Every public type needs full `///` documentation**: what it is, why it exists, and its boundaries and errors, including `# Errors` / `# Examples` sections.
4. **Conventional commits**: `feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`. Commit bodies are written in Chinese (the maintainer's working language).
5. **Frontend styles and copy**: styles use CSS variables (`--bg-card` / `--accent` / `--text-dim`), and all user-facing copy lives in `ui/src/i18n/index.ts` — **the zh and en dictionaries must be updated together**.

## 4. Adding a checker

Checkers are pluggable; adding one takes three steps and no engine changes:

1. Create a file under `crates/rb-core/src/checker/` implementing the `Checker` trait:

   ```rust,ignore
   fn id(&self) -> &'static str;                       // stable unique id
   fn category(&self) -> Category;                     // owning dimension
   fn plan(&self, ctx: &ScanContext) -> ScanPlan;      // declare data slices (idempotent)
   fn check(&self, ctx: &ScanContext) -> Vec<Finding>; // pure computation, same input → same output
   ```

2. Export it from `checker/mod.rs` and register it in `src/bin/rb.rs` and `crates/rb-app/src/commands.rs`; add the id to the frontend's `ALL_CHECKER_IDS`.
3. Add tests covering the four classes: **hit / no-hit / threshold boundary / ignore rules**, following the existing `TestRepo` fixture style.

Conventions:

- `check` is a pure function: no clock reads, no I/O. Where "now" is needed, use the newest commit time in the repository as the reference.
- Evidence containing secrets is **masked at construction time** — mask once, masked everywhere.
- Dimensions without a registered checker score `None` (shown as N/A in the UI); never fabricate a full score.

## 5. Submitting a PR

- One PR does one thing; describe the motivation, the approach and how you verified it.
- CI runs the checkup on this repo itself: `rb scan . --ci` fails on any Critical finding, so keep it at zero Critical.
- The repository is mirrored to both AtomGit and GitHub; after merging, the maintainer pushes the **same commit to both remotes**.

## 6. Reporting issues

Use the issue templates and include as much as you can:

- characteristics of the repository that reproduces it (size, branch count, LFS usage — desensitise as needed)
- RepoBalance version / OS / how you installed it
- the checkup report JSON (`rb scan <path> --json`)

> **Security issues must not be filed publicly** — use the private channels in [`SECURITY.md`](SECURITY_EN.md).
> Rule accuracy problems (a missed secret type, or a sample value flagged as real) are ordinary issues.

## 7. Code of conduct

By participating you agree to follow [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT_EN.md) (adapted from Contributor Covenant 2.1). Keep discussions friendly and focused on the technical merits.

## 8. Versions and releases

Notable changes are recorded in [`CHANGELOG.md`](CHANGELOG.md) (Keep a Changelog + SemVer, written in Chinese).
Release flow: merge to `master` → tag `vX.Y.Z` → the packaging workflow produces installers for each platform → push the tag to both remotes.
