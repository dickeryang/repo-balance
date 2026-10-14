#!/usr/bin/env bash
# 生成仓衡离线漏洞库索引（vuln-index.jsonl）。
#
# 用法：
#   bash scripts/build-vuln-index.sh [输出路径]
# 默认输出：./vuln-index.jsonl（随后手工放到应用数据目录，见下方说明）
#
# 行为：
#   1. 从 OSV（https://osv.dev，Google 托管的公开漏洞库）下载各生态全量归档；
#   2. 转换成仓衡的紧凑 JSONL 格式（每行一条公告），只保留匹配与展示所需字段；
#   3. 输出到目标路径。
#
# 说明（重要）：
#   - 本脚本是**开发者/运维手工执行**的离线数据准备步骤，不在 cargo build 或
#     CI 中运行——仓衡运行时永远零网络，漏洞库是「带过来的数据」而非「联网查询」；
#   - 生成产物放到应用数据目录后桌面端自动载入；CLI 用 --vuln-db FILE 指定；
#   - 未放置索引时依赖维度照常做结构检查，不会报错。
#
# 依赖：curl、unzip、python3（macOS/Linux 常见环境均自带）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/vuln-index.jsonl}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

info() { echo "[漏洞库] $*"; }
die() { echo "[漏洞库] 错误: $*" >&2; exit 1; }

for tool in curl unzip python3; do
  command -v "$tool" >/dev/null 2>&1 || die "缺少依赖: $tool"
done

# OSV 生态名 → 归档路径段。仅收录仓衡依赖维度支持的五个生态。
ECOSYSTEMS="npm PyPI Go crates.io Maven"

BASE_URL="https://osv-vulnerabilities.storage.googleapis.com"

for eco in $ECOSYSTEMS; do
  # OSV 归档路径对大小写与点号敏感：crates.io / PyPI / Maven / Go / npm
  info "下载 $eco 归档…"
  curl -fsSL --retry 3 --retry-delay 2 \
    "$BASE_URL/$eco/all.zip" -o "$TMP/$eco.zip" \
    || die "下载 $eco 归档失败（检查网络或 OSV 可用性）"
  mkdir -p "$TMP/$eco"
  unzip -q -o "$TMP/$eco.zip" -d "$TMP/$eco" || die "解压 $eco 归档失败"
done

info "转换为紧凑 JSONL…"
python3 - "$TMP" "$OUT" <<'PY'
import json, os, sys, glob

tmp, out = sys.argv[1], sys.argv[2]

# OSV 生态名 → 仓衡索引内的短名（须与 rb-core VulnEcosystem 的 serde 名一致）
ECO_MAP = {
    "npm": "npm",
    "PyPI": "pypi",
    "Go": "go",
    "crates.io": "cargo",
    "Maven": "maven",
}
# OSV severity（database_specific.severity）→ 仓衡 Severity
SEV_MAP = {
    "CRITICAL": "critical",
    "HIGH": "critical",
    "MODERATE": "warning",
    "LOW": "info",
}

records = {}

def sev_of(adv, affected):
    """严重程度取值顺序：affected.database_specific → 顶层 database_specific → 默认。"""
    for scope in (affected.get("database_specific") or {}, adv.get("database_specific") or {}):
        s = (scope or {}).get("severity")
        if s in SEV_MAP:
            return SEV_MAP[s]
    # 无显式等级时按 CVSS 分数粗分（>=7 critical，>=4 warning）
    for item in adv.get("severity") or []:
        score = item.get("score", "")
        if "/CVSS:" in score:
            try:
                base = float(score.split("/")[-1].split("/")[0])
            except (ValueError, IndexError):
                continue
            if base >= 7.0:
                return "critical"
            if base >= 4.0:
                return "warning"
            return "info"
    return None

for eco_dir in os.listdir(tmp):
    eco = ECO_MAP.get(eco_dir)
    if not eco:
        continue
    for path in glob.glob(os.path.join(tmp, eco_dir, "*.json")):
        try:
            with open(path, encoding="utf-8") as fh:
                adv = json.load(fh)
        except (OSError, json.JSONDecodeError):
            continue  # 单条坏数据跳过，不中断整批
        aid = adv.get("id")
        if not aid:
            continue
        summary = (adv.get("summary") or "").replace("\n", " ").strip()
        if len(summary) > 160:
            summary = summary[:157] + "..."
        for affected in adv.get("affected") or []:
            pkg = (affected.get("package") or {}).get("name")
            if not pkg:
                continue
            ranges = []
            for r in affected.get("ranges") or []:
                # GIT 区间按提交哈希界定，版本比较无意义，跳过
                if r.get("type") == "GIT":
                    continue
                introduced = fixed = last = None
                for ev in r.get("events") or []:
                    if "introduced" in ev:
                        introduced = ev["introduced"]
                    elif "fixed" in ev:
                        fixed = ev["fixed"]
                    elif "last_affected" in ev:
                        last = ev["last_affected"]
                if introduced == "0":
                    introduced = None  # "0" 等价于不限下界，省字节
                if introduced or fixed or last:
                    ranges.append({"introduced": introduced, "fixed": fixed, "last_affected": last})
            key = (eco, pkg.lower(), aid)
            rec = records.setdefault(key, {
                "id": aid,
                "eco": eco,
                "pkg": pkg,
                "ranges": [],
                "sev": sev_of(adv, affected),
                "summary": summary,
            })
            for r in ranges:
                if r not in rec["ranges"]:
                    rec["ranges"].append(r)
            # 同一公告跨多个 affected 时取更严重的等级
            s = sev_of(adv, affected)
            if s == "critical":
                rec["sev"] = "critical"

rows = [records[k] for k in sorted(records)]
with open(out, "w", encoding="utf-8") as fh:
    for r in rows:
        # 去掉空值字段，压缩体积
        r["ranges"] = [{k: v for k, v in rg.items() if v is not None} for rg in r["ranges"]]
        if r["sev"] is None:
            del r["sev"]
        if not r["summary"]:
            del r["summary"]
        fh.write(json.dumps(r, ensure_ascii=False, separators=(",", ":")) + "\n")

print(f"{len(rows)}")
PY

COUNT=$(wc -l < "$OUT" | tr -d ' ')
SIZE=$(du -h "$OUT" | awk '{print $1}')
info "已生成: $OUT（$COUNT 条公告，约 $SIZE）"
info "桌面端：复制到应用数据目录并命名为 vuln-index.jsonl"
info "CLI   ：rb scan . --ci --vuln-db $OUT"
