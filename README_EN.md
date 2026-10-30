# 仓衡 RepoBalance

[简体中文](README.md) | English

[![AtomGit G-Star](https://atomgit.com/dickeryang/repo-balance/star/new_badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Star](https://atomgit.com/dickeryang/repo-balance/star/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Fork](https://atomgit.com/dickeryang/repo-balance/fork/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Download](https://atomgit.com/dickeryang/repo-balance/download/badge.svg)](https://atomgit.com/dickeryang/repo-balance/releases)
[![CI](https://github.com/dickeryang/repo-balance/actions/workflows/ci.yml/badge.svg)](https://github.com/dickeryang/repo-balance/actions/workflows/ci.yml)

RepoBalance ("仓衡", *cāng héng* — a granary scale) is a cross-platform desktop app that runs a structured health checkup on your local Git repositories: large files, zombie branches, commit habits, dependency health and leaked secrets, producing an exportable checkup report. It was built independently with the Huawei Cloud CodeArts coding agent.

**Everything stays on your machine** — scanning never touches the network, there is no telemetry, and secret evidence is masked before it ever leaves the Rust core. The single exception is the update check you trigger yourself (off by default).

## Hosting & Sync

The canonical hosting for China is **AtomGit**, and the project joins the [AtomGit G-Star Program](https://atomgit.com/GitCode/G-Star_guiding).

<a href="https://atomgit.com/dickeryang/repo-balance"><img src="docs/AtomGit-logo.svg" alt="AtomGit" width="28"></a> [Hosted on AtomGit: https://atomgit.com/dickeryang/repo-balance](https://atomgit.com/dickeryang/repo-balance)

| Platform | URL | Notes |
|---|---|---|
| AtomGit (canonical in China) | https://atomgit.com/dickeryang/repo-balance | Official host; where the G-Star certification lives |
| GitHub (international mirror) | https://github.com/dickeryang/repo-balance | Kept in sync with AtomGit |

> **Sync policy:** every release pushes the same commit to both remotes. G-Star requires that a mirrored platform never falls more than one month behind, otherwise AtomGit may revoke the certification — so "push to both remotes" is a mandatory release step here.

## Demo

**Pick a repository and start the checkup:**

![Select repository demo](docs/repobalance-git-demo-repo-select.gif)

**Scan history comparison (new / resolved findings):**

![History comparison demo](docs/repobalance-git-demo-history-compare.gif)

### Generate the demo repository (see every checker in action)

`demo-repo/` is a synthetic sample generated locally (it is git-ignored, so a fresh clone does not contain it). One command creates it:

```bash
bash scripts/build-demo-fixture.sh
```

The script is **idempotent** and never rewrites or amends existing commits. It produces four kinds of synthetic data:

| Fixture | Checker it exercises |
|---|---|
| `big.bin` (5 MiB, committed and still in the worktree) | Big files · Warning |
| `.env` (with assignments) | Leaked secrets · Warning |
| `old-branch` (holds an unmerged `wip` commit) | Zombie branches · Info |
| `leaked.txt` (`DEMO KEY` placeholder block) | The secrets stop-word allowlist (produces no finding) |

Then either click "试试演示仓库 demo-repo" in the app, or scan it from the CLI:

```bash
cargo run -p rb-core --bin rb scan ./demo-repo
```

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2 (native Rust WebView, zero Electron) |
| Frontend | Vue 3 + TypeScript + Pinia + Vite 4 |
| Checker core | Rust (git2 + rayon + serde + thiserror) |
| IPC | Tauri IPC (invoke commands + emit events) |

## Prerequisites

| Tool | Minimum | Notes |
|---|---|---|
| Rust | 1.88+ | Dependency-graph MSRV floor (`darling` / `plist` / `time` declare 1.88); `rustup update stable` |
| Node.js | 14.18+ | Required by Vite 4 |
| npm | 6+ | or pnpm / yarn |
| Windows SDK | 10+ | Windows builds only (`dbghelp.lib` + `advapi32.lib`) |

### Install the Tauri CLI

```bash
cargo install tauri-cli --version "^2"
```

## Development

### 1. Install frontend dependencies

```bash
cd ui
npm install --registry https://registry.npmjs.org/
cd ..
```

> If your npm default registry is an internal mirror, pass `--registry https://registry.npmjs.org/` — packages such as `@tauri-apps/api` must come from the official registry.

### 2. Start the dev server

**Option A (recommended): one-shot Tauri dev**

```bash
cargo tauri dev
```

This starts the Vite dev server (port 5173) plus Rust hot reload and opens a 1200×800 desktop window.

**Option B: separate terminals**

```bash
# Terminal 1: frontend dev server
cd ui && npm run dev

# Terminal 2: Rust side
cargo run -p rb-app
```

### 3. Quality gates

These are exactly the checks CI runs ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) — please run them locally before pushing:

```bash
# Rust formatting (rustfmt)
cargo fmt --all -- --check

# Rust lint (warnings are errors)
cargo clippy --all-targets --workspace -- -D warnings

# Rust tests
cargo test --workspace

# Frontend lint + build
cd ui && npm run lint && npm run build
```

> CI also runs two separate jobs: **dependency licence & vulnerability audit** (cargo-deny, configured in [`deny.toml`](deny.toml)) and a **rb-core coverage summary** (cargo-llvm-cov).

## Build & Release

### Production build

```bash
cargo tauri build
```

This runs, in order:

1. `npm run build` (frontend output → `ui/dist/`)
2. `cargo build --release -p rb-app --features custom-protocol` (optimized Rust build, frontend assets embedded)
3. Platform installer packaging

Artifacts land in `target/release/bundle/`:

| Platform | Artifact | Path |
|---|---|---|
| Windows | `.msi` installer + `.exe` | `target/release/bundle/msi/` |
| Windows | NSIS `.exe` installer | `target/release/bundle/nsis/` |
| macOS | `.dmg` + `.app` | `target/release/bundle/dmg/` |
| Linux | `.deb` + `.AppImage` | `target/release/bundle/deb/` |

### Installing and first launch (unsigned builds)

Release artifacts are **not code-signed** (the usual state for open-source projects), so the OS will block the first launch:

| Platform | Prompt | Workaround |
|---|---|---|
| macOS | "Apple could not verify … is free of malware" | Right-click the app → Open; or `xattr -dr com.apple.quarantine "/Applications/仓衡.app"` |
| Windows | SmartScreen warning | "More info" → "Run anyway" |

If that is not acceptable to you, build from source — every build step above is reproducible locally.

### Frontend only (no desktop bundle)

```bash
cd ui && npm run build
```

Produces `ui/dist/` (`index.html` + `assets/`), usable as a web preview or embedded in another container.

### Rust binary only (no installer)

```bash
cargo build --release -p rb-app --features custom-protocol
```

Produces `target/release/rb-app` (`rb-app.exe` on Windows) with the frontend assets already embedded — runnable directly.

> ⚠️ `--features custom-protocol` is **required**. Without it Tauri compiles in dev mode (`tauri-build` sets `cfg(dev)`), which does not embed the frontend and instead loads it from `devUrl` (`http://localhost:5173`); with no dev server running the window renders blank.

## CLI Mode (no GUI)

`rb-core` also compiles as a standalone command-line tool with no Tauri dependency:

```bash
cargo run -p rb-core --bin rb scan ./demo-repo
```

It prints the repository summary plus the checkup findings (severity, masked evidence, remediation advice).

Remote repositories go through the same pipeline (shallow-cloned into a temp directory, cleaned up on exit, nothing persisted locally):

```bash
cargo run -p rb-core --bin rb scan-url https://host/repo.git
```

Ignore rules can also be **validated read-only** (same semantics as the GUI editor's dry run; nothing is written):

```bash
cargo run -p rb-core --bin rb ignore ./demo-repo
```

It prints how many user rules and built-in defaults apply, and lists rules that would silently do nothing (empty pattern / backslash / duplicate). Exit code 1 when any suspicious rule is found, so it fits CI or a pre-commit hook.

## Project Structure

```
repo-balance/
├── crates/
│   ├── rb-core/              # Checker core (zero Tauri dependency)
│   │   ├── src/
│   │   │   ├── checker/      # Checker trait + registry + slice plan + context + 6 built-in checkers
│   │   │   ├── engine/       # Scan orchestration (plan → merge → collect → check)
│   │   │   ├── git/          # Read-only git2 facade (no type leakage)
│   │   │   ├── model/        # Data models (RepoSnapshot / Finding / ScanReport)
│   │   │   ├── rules.rs      # Custom rule model (validation + compile to linear-time regex)
│   │   │   └── bin/rb.rs     # CLI entry point
│   │   └── build.rs          # Windows advapi32 link fixup
│   └── rb-app/              # Tauri 2 desktop shell
│       ├── src/
│       │   ├── main.rs       # Tauri Builder entry
│       │   ├── commands.rs   # IPC commands (scan / history / rules / ignore rules / export)
│       │   ├── dto.rs        # camelCase DTO mirror layer
│       │   ├── error.rs      # Error mapping
│       │   └── state.rs      # Scan state management
│       ├── capabilities/     # Minimal permission set
│       ├── icons/            # App icons
│       ├── tauri.conf.json   # Tauri config (CSP / window / bundling / update endpoint & public key)
│       ├── tauri.updater.conf.json # Release-only overlay (createUpdaterArtifacts, merged in CI)
│       └── build.rs          # tauri-build
├── ui/                       # Vue 3 frontend project
│   ├── src/
│   │   ├── views/            # Views (connect / scan / report / history / batch dashboard / settings)
│   │   ├── components/       # Shared components (TopBar / RadarChart / TrendChart)
│   │   ├── stores/           # Pinia state stores
│   │   ├── ipc/              # IPC wrapper (commands + events)
│   │   ├── utils/            # Pure helpers (kept out of rendering so they are unit-testable)
│   │   ├── styles/           # Theme styles (14 CSS variables)
│   │   ├── test/             # Test environment (in-memory localStorage)
│   │   └── types/            # TS type mirrors
│   ├── package.json
│   ├── vite.config.ts
│   └── vitest.config.ts      # Vitest config (node env, store + pure helper unit tests)
├── docs/                     # Documentation (roadmap / custom rules / ignore rules / CI integration / demo GIFs)
│   ├── roadmap.md            # Authoritative roadmap (current state / goals / acceptance)
│   ├── custom-rules.md       # Custom rule JSON format and behavior (Chinese)
│   ├── ignore-rules.md       # Ignore-rule syntax, built-in defaults, editor & CLI semantics
│   ├── distribution.md       # Release & distribution handbook (update signing / package manifests / release notes)
│   └── ci-integration.md     # CI integration guide
├── design/                   # Brand assets
│   ├── icon.svg              # App icon source
│   ├── preview.png           # 1024×1024 icon preview (usable directly as a repo avatar)
│   └── preview-64.png        # 64×64 icon
├── scripts/                  # Demo fixture / advisory index / release notes / package manifests
└── Cargo.toml                # Workspace root
```

## Security Design

- **CSP**: `default-src 'self'` (no external resource loading)
- **Minimal permissions**: `core:default` + `dialog:allow-open` + `core:event:default` + `updater:default` + `process:default` (no fs/shell)
- **Path sources**: system directory picker only — no free-text path input
- **Local writes**: besides reports the user exports explicitly, the only files written are `scan-history.json` in the app data directory and — the one write into the repository being checked — `.repobalance-ignore` when the user saves ignore rules (they must travel with the repository); the git object database and commit history are never touched
- **Offline by default**: scanning, scoring, history, export and advisory-DB import make no network request; the only outbound call is the manual update check (off by default, optional check-on-startup), carrying no repository content or device identifiers
- **Signed updates**: installers are signed with a minisign private key in the release pipeline and the public key is embedded; a missing or mismatched signature is rejected
- **Masked evidence**: evidence is masked when constructed in `rb-core`; export/IPC never restores it
- **Zero Tauri dependency in `rb-core`**: the core compiles as a standalone CLI, decoupled from any UI framework

## Built-in Checkers

| Checker | Dimension | Rules |
|---|---|---|
| `big-files` | Structure | Large files in the worktree and in history (threshold configurable, 1 MiB default) |
| `secrets` | Security | Private-key headers, AWS access keys, GitHub/GitLab tokens, `.env` assignments, high-entropy secrets (regex + Shannon entropy, with a stop-word allowlist) |
| `zombie-branches` | Branches | Merged but undeleted, inactive beyond N days (90 by default), local branches without upstream |
| `commit-habits` | History | Oversized commits (>50 files Warning / >200 Critical), low-quality message ratio, late-night commit ratio (per commit timezone) |
| `deps-health` | Dependencies | Missing / uncommitted / deleted lockfiles, multiple package managers in one ecosystem |
| `conflict-markers` | Structure | Leftover `<<<<<<<` / `=======` / `>>>>>>>` markers in the worktree and in history (three false-positive guards) |
| `custom-rules` | per rule | User-defined JSON rules (regex + path glob + severity + category); only registered when rules exist |

Dimensions with no registered checker score `None` (shown as **N/A**) instead of a fake 100, and the total score averages only the covered dimensions.

## Roadmap

> The full roadmap (current state, phase goals, acceptance criteria) lives in [`docs/roadmap.md`](docs/roadmap.md) (Chinese).
> The list below is a summary only; unimplemented features are never marked as done — documentation never runs ahead of the code.

### Shipped in the current version (v0.1.x)

- [x] **All six checkers live** — large files (structure), secrets (security), zombie branches (branches), commit habits (history), dependency health (deps), merge-conflict leftovers (structure): every dimension produces real findings
- [x] **Merge-conflict leftovers checker** — detects leftover `<<<<<<<` / `=======` / `>>>>>>>` markers in both the worktree and commit history; three false-positive guards (a full marker triple is required, a stop-word allowlist for doc examples, shared ignore rules), and history findings say "usually no action needed" rather than pushing a history rewrite
- [x] **Secrets checker** — private-key headers / AWS access key / GitHub & GitLab tokens / `.env` assignments / high-entropy secrets, dual-channel (regex + Shannon entropy), evidence masked at construction, stop-word allowlist for sample values
- [x] **Zombie branches checker** — merged but undeleted, inactive beyond N days (90 default, configurable), no upstream tracking
- [x] **Commit habits checker** — oversized commits (>50 files Warning / >200 Critical), low-quality message ratio, late-night ratio (per commit timezone)
- [x] **Dependency health checker** — missing / uncommitted / deleted lockfiles, mixed package managers, plus known-vulnerability findings once an offline advisory index is present (parses 8 manifest/lockfile kinds with cross-ecosystem version matching; falls back to structural checks with no index)
- [x] **Scoring fix** — dimensions without a checker show N/A instead of 100; the total score averages covered dimensions only
- [x] **Report export** — JSON / Markdown / HTML (self-contained, inline SVG radar chart)
- [x] **Scan history** — persisted in `scan-history.json` under the app data directory (legacy localStorage data is migrated automatically); compare two scans (new / resolved findings); import / export as a JSON backup
- [x] **Ignore rules** — `.repobalance-ignore` with negated rules (`!pattern`); built-in defaults for build output (`target/`, `node_modules/`, …) that can be negated
- [x] **Ignore rules editor (settings page)** — pick a repository and read / edit its root `.repobalance-ignore` in place; "Dry run" evaluates the unsaved text in memory (built-in defaults + your rules, exactly as the scan does) and reports how many files would be ignored, their share, total size and path samples, while flagging rules that would silently do nothing (empty pattern / backslash / duplicate); saving is atomic, and `rb ignore <path>` validates read-only with the same semantics (syntax and semantics: `docs/ignore-rules.md`, Chinese)
- [x] **CI integration** — GitHub Actions pipeline running the checkup in CLI mode and uploading the report
- [x] **i18n** — Chinese / English UI toggle
- [x] **Large-repo throttle** — limit history traversal to the newest N commits / last N days (CLI `--max-commits` / `--since-days`, GUI "History Scope"); limited scans are labelled in the CLI, report page and exports
- [x] **Scan scale estimate** — the connect view shows commits / files / size and an estimated duration (object count and duration are extrapolated from a sample); large repos get a suggested limit that can be applied in one click
- [x] **Custom rules** — define team-specific checks in JSON (regex + path glob + severity + category + advice) and turn your own conventions into reproducible findings; managed via GUI forms with JSON import / export, and `--rules FILE` on the CLI; regex matching is linear-time, matched samples are masked, and each rule is capped at 50 file findings with an honest truncation notice (format: [`docs/custom-rules.md`](docs/custom-rules.md), Chinese)
- [x] **Remote repository checkup** — shallow-clone a URL into a temp directory (`TempDir` RAII cleans up automatically) and reuse the local pipeline; the report path is the URL. CLI `rb scan-url <url> [--depth N]`, plus a remote-URL entry on the connect view. HTTPS URLs work out of the box; `git@host:` relies on the `insteadOf` rewrite in your git config, and `ssh://` is not enabled (it reports back with a "use HTTPS instead" hint)
- [x] **Health trend analysis** — the history view charts "total + five dimensions" over time per repository (inline SVG, no chart dependency); N/A dimensions break the line instead of being interpolated, legend chips toggle curves, and per-point tooltips show time and value
- [x] **Bulk multi-repository checkup** — select several folders at once to build a sequential queue where one failing repository never aborts the batch; the dashboard shows batch progress plus per-repo score / findings / criticals / duration ranked by score, averages count finished repositories only, and finished runs are written to history and can jump straight back into their report
- [x] **Frontend unit tests** — Vitest covers the store's config persistence / history merge and migration / score semantics / trend aggregation / batch scheduling state machine, plus the ignore-rules editor's pure helpers and the update state machine (44 cases), and runs as part of the CI quality gate
- [x] **Dedicated settings page** — checker toggles / thresholds / history scope / custom rules / the ignore-rules editor moved out of the connect view into a settings page (top-bar "Settings"), leaving connect with "pick repo → scale estimate → start scan"; settings are global and stored locally
- [x] **Offline advisory DB management** — import a JSONL index from the settings page and see its status (entries / updated / size); validation runs before writing, so a corrupt or empty index never overwrites a working one; purely local, no network access
- [x] **In-app updates** — the settings page checks GitHub Releases on demand, shows release notes and download progress, and restarts after installing; installers are verified with a minisign signature. **No background polling** — offline by default, a request happens only when you click the button or explicitly enable "check on startup" (see [`docs/distribution.md`](docs/distribution.md))
- [x] **Distribution manifests & release notes** — `scripts/gen-packaging-manifests.sh` turns the real release asset checksums into Homebrew Cask / Scoop / WinGet manifests that the release workflow attaches to the release; release bodies are extracted from CHANGELOG by `scripts/release-notes.sh`, so release notes and the changelog cannot drift apart

### Next (v0.2 ~ v0.3)

- [ ] **PDF export** (reports contain Chinese text; CJK font embedding needs to be solved)

### Mid / long term (v0.5 → v1.0)

- [ ] **Bundled advisory DB** — the settings page can import a JSONL index and show its status, but the index is not bundled with the installer and is never auto-updated over the network (it is still generated by `scripts/build-vuln-index.sh`, so a checkup never becomes a network query)
- [ ] **Official GitHub Action** — package the CLI as a CI action that runs a checkup on PRs and comments the result
- [ ] **Team report collaboration** — Report as Code, closing remediation items through PR review

## Contributing

Issues and PRs are welcome. The full workflow, code discipline and the three-step recipe for adding a checker are in [`CONTRIBUTING_EN.md`](CONTRIBUTING_EN.md) ([简体中文](CONTRIBUTING.md)).

- Before filing anything, check [`SUPPORT.md`](SUPPORT.md) (Chinese — unsigned builds, false positives, how the score is computed)
- Bugs and feature requests: use the [issue templates](.github/ISSUE_TEMPLATE)
- Pull requests: check off the validation list in the [PR template](.github/PULL_REQUEST_TEMPLATE.md)
- Community standards: [`CODE_OF_CONDUCT_EN.md`](CODE_OF_CONDUCT_EN.md) ([简体中文](CODE_OF_CONDUCT.md))
- Security issues: **do not** open a public issue — use the private channels in [`SECURITY_EN.md`](SECURITY_EN.md) ([简体中文](SECURITY.md))
- Governance and roles: [`GOVERNANCE.md`](GOVERNANCE.md) (Chinese) | Contributors: [`AUTHORS.md`](AUTHORS.md) (Chinese)
- Release notes: [`CHANGELOG.md`](CHANGELOG.md) (Chinese)

## License

Released under the [Apache License 2.0](LICENSE).
