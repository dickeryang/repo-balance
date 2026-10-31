#!/usr/bin/env python3
"""把仓衡体检报告发表 / 更新到 PR 评论（官方 GitHub Action 的可选步骤）。

设计口径：
- 评论带稳定标记 `<!-- repobalance:report -->`，**原位更新**而非每次新发，
  避免 CI 反复运行把 PR 刷成评论区；
- 只展示报告里已有的字段（id / severity / title / 总分 / 计数 / 范围），
  evidence 在 CLI 出口就已掩码，这里不做二次加工、更不还原；
- fork PR 的默认 token 只读，API 返回 403 时降级为警告退出 0——
  评论发不出去不应让本来通过的体检变红；其余 API 错误按失败处理。

用法：
  python3 action-comment.py            # 读取 RB_REPORT 并 upsert 评论
  python3 action-comment.py --print    # 仅打印将发表的 Markdown（本地自测）
"""

from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

MARKER = "<!-- repobalance:report -->"
API_BASE = "https://api.github.com"
MAX_BODY_CHARS = 60_000  # GitHub 评论上限 65536，留余量
TOP_FINDINGS = 10

SEVERITY_ICON = {"critical": "🔴", "warning": "🟠", "info": "🔵"}


def format_comment(report: dict) -> str:
    """报告 JSON → Markdown。报告字段口径见 `rb scan --json`。"""
    total = report.get("totalScore")
    critical = report.get("critical", 0)
    warning = report.get("warning", 0)
    info = report.get("info", 0)
    scope = report.get("scope")
    findings = report.get("findings", [])

    if total is not None:
        head = f"**总分 {total}/100**（未覆盖维度不参与平均）"
    else:
        head = "**总分未定**（本次扫描没有覆盖任何评分维度）"

    lines = [
        MARKER,
        "## 🩺 仓衡体检报告",
        "",
        f"{head} · 🔴 Critical {critical} · 🟠 Warning {warning} · 🔵 Info {info}",
    ]
    if scope:
        lines.append(f"> 扫描范围：{scope}（受限扫描的结论不等同于全量结论）")
    lines.append("")

    if findings:
        lines.append("| 严重度 | 问题 |")
        lines.append("|---|---|")
        for f in findings[:TOP_FINDINGS]:
            icon = SEVERITY_ICON.get(f.get("severity", ""), "•")
            lines.append(f"| {icon} {f.get('severity', '')} | `{f.get('id', '')}` {f.get('title', '')} |")
        rest = findings[TOP_FINDINGS:]
        if rest:
            lines.append("")
            lines.append(f"<details><summary>其余 {len(rest)} 条结论</summary>")
            lines.append("")
            lines.append("| 严重度 | 问题 |")
            lines.append("|---|---|")
            for f in rest:
                icon = SEVERITY_ICON.get(f.get("severity", ""), "•")
                lines.append(f"| {icon} {f.get('severity', '')} | `{f.get('id', '')}` {f.get('title', '')} |")
            lines.append("")
            lines.append("</details>")
    else:
        lines.append("没有结论：五个维度全部通过。")

    lines += [
        "",
        "完整 JSON 见 workflow artifact `repo-balance-report`。",
        f"> 由 [RepoBalance]({'https://github.com/dickeryang/repo-balance'})"
        " 在本地扫描生成，不上传仓库内容；evidence 已掩码。",
        "",
    ]
    return "\n".join(lines)


def api_request(method: str, url: str, token: str, payload: dict | None = None) -> dict:
    body = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = urllib.request.Request(url, data=body, method=method)
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    req.add_header("User-Agent", "repobalance-action")
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req) as resp:
        return json.load(resp) if resp.length else {}


def upsert_comment(repo: str, pr_number: int, token: str, body: str) -> None:
    """找到带标记的历史评论则 PATCH，否则 POST。"""
    existing_id = None
    for page in (1, 2, 3):
        comments = api_request(
            "GET",
            f"{API_BASE}/repos/{repo}/issues/{pr_number}/comments?per_page=50&page={page}",
            token,
        )
        for c in comments:
            if MARKER in c.get("body", ""):
                existing_id = c["id"]
                break
        if existing_id or len(comments) < 50:
            break

    if existing_id:
        api_request("PATCH", f"{API_BASE}/repos/{repo}/issues/comments/{existing_id}", token, {"body": body})
        print(f"[repobalance] 已更新既有评论 #{existing_id}")
    else:
        api_request("POST", f"{API_BASE}/repos/{repo}/issues/{pr_number}/comments", token, {"body": body})
        print("[repobalance] 已发表体检评论")


def main() -> int:
    argv = sys.argv[1:]
    report_path = os.environ.get("RB_REPORT", "repobalance-report.json")
    try:
        with open(report_path, encoding="utf-8") as f:
            report = json.load(f)
    except (OSError, ValueError) as e:
        print(f"[repobalance] 错误: 无法读取报告 {report_path}: {e}", file=sys.stderr)
        return 1

    body = format_comment(report)[:MAX_BODY_CHARS]

    if "--print" in argv:
        print(body)
        return 0

    missing = [k for k in ("RB_TOKEN", "RB_REPO", "RB_PR_NUMBER") if not os.environ.get(k)]
    if missing:
        print(f"[repobalance] 错误: 缺少环境变量 {', '.join(missing)}", file=sys.stderr)
        return 1

    token = os.environ["RB_TOKEN"]
    repo = os.environ["RB_REPO"]
    pr_number = os.environ["RB_PR_NUMBER"]

    try:
        upsert_comment(repo, int(pr_number), token, body)
    except urllib.error.HTTPError as e:
        # fork PR 的默认 token 只读：评论失败不应连累体检结论
        if e.code in (401, 403, 404):
            print(f"[repobalance] 警告: 无权发表评论（HTTP {e.code}）；"
                  "需要 pull-requests: write 权限或改用 PAT", file=sys.stderr)
            return 0
        print(f"[repobalance] 错误: GitHub API 返回 HTTP {e.code}", file=sys.stderr)
        return 1
    except urllib.error.URLError as e:
        print(f"[repobalance] 错误: 无法访问 GitHub API: {e.reason}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
