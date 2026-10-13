# Security Policy

[简体中文](SECURITY.md) | English

RepoBalance is a **fully local** desktop tool: it never talks to the network, never uploads data, and never executes code from the repositories it scans (it only reads Git objects and worktree files). Security issues therefore fall into two very different categories:

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
- **Minimal permissions**: `core:default` + `dialog:allow-open` + `core:event:default` — **no** `fs` / `shell`
- **Single source of paths**: only the system directory picker; no free-text path input
- **Zero network**: no HTTP client is linked in, and no outbound connection is ever made
- **Masked evidence**: secret values are masked while the evidence is built inside `rb-core`; neither IPC nor exports restore them
- **Read-only scanning**: repository scripts are never executed and scanned repositories are never modified
