# Security Policy

[简体中文](SECURITY.md) | English

RepoBalance is a **local-first** desktop tool: scanning and reporting never touch the network, never upload data, and never execute code from the repositories it scans (it only reads Git objects and worktree files). The only outbound behaviour is the update check the user triggers explicitly from the settings page (off by default); see [Network behaviour](#network-behaviour). Security issues therefore fall into two very different categories:

| Category | Examples | Use this process? |
|---|---|---|
| **A. Defects in the program itself** | crash / out-of-bounds / command injection triggered by a crafted repository; over-broad permissions; CSP bypass | ✅ report privately, see below |
| **B. Missed or false findings** | a secret type we do not detect; a sample value flagged as a real secret; an off-by-one in a threshold | ❌ a normal issue is fine (still welcome) |

## Supported versions

| Version | Accepts security fixes |
|---|---|
| Latest release (whatever `master` points at) | ✅ |
| Anything older | ❌ please upgrade first, then report |

## How to report (please do not file publicly)

For category A, do **not** use public issues, discussions or PRs. Use one of these private channels:

1. **Preferred**: GitHub private vulnerability reporting — repository `Security` tab → `Report a vulnerability`
   (https://github.com/dickeryang/repo-balance/security/advisories/new — supports private collaboration and CVE requests)
2. **Alternative**: email `dickeryang@foxmail.com` with a subject starting with `[SECURITY]`

## What to include

- affected version and OS (including CPU architecture)
- the kind of issue and its impact (crash / data exposure / privilege escalation / DoS …)
- **how to construct a minimal reproducing repository** (describe it in a script; do not attach real confidential data)
- reproduction steps and expected vs. actual behaviour
- a suggested fix if you have one

## Our commitments

| Stage | Target |
|---|---|
| Acknowledge receipt | within 72 hours |
| Initial assessment (accepted? severity?) | within 7 days |
| Fix and patch release | negotiated by severity; Critical first |
| Public credit | after the fix ships (if you want to be named) |

## Deliberate security boundaries

These constraints are intentional and are not vulnerabilities in themselves. If you can bypass any of them, report it as category A:

- **CSP is `default-src 'self'`**: no external resource may be loaded (fonts, images, CDNs included)
- **Minimal permissions**: `core:default` + `dialog:allow-open` + `core:event:default` + `updater:default` + `process:default` — **no** `fs` / `shell`
- **Single source of paths**: only the system directory picker; no free-text path input
- **Masked evidence**: secret values are masked while the evidence is built inside `rb-core`; neither IPC nor exports restore them
- **Read-only scanning**: repository scripts are never executed and scanned repositories are never modified; the one exception is `.repobalance-ignore` saved from the settings page (ignore rules must travel with the repository) — the git object database and commit history are never touched

## Network behaviour

RepoBalance has exactly one network path, and it is off by default:

- **Scanning is fully offline**: scanning, scoring, history, export and advisory-index import
  never create an HTTP client and never open an outbound connection. Neither `rb-core` nor
  the CLI links a network stack.
- **The only outbound request comes from the update check** (`tauri-plugin-updater`) and only
  when the user clicks "Check for updates" or explicitly enables "Check for updates on
  startup" (stored on this machine, off by default). It fetches version metadata
  (`latest.json`) and the installer only — **no** repository content, paths, scan results or
  device identifiers are sent, there is no background polling and no telemetry.
- **Update packages must be signed**: installers are signed with a minisign private key in the
  release pipeline, and the public key is embedded in `tauri.conf.json`. A missing or
  mismatched signature is rejected, so arbitrary downloaded binaries are never executed.
- **Disabled means no traffic**: with the default settings the process makes no outbound
  connection at all — verifiable with a firewall, `nettop` or a packet capture.
