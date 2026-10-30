#!/usr/bin/env bash
# 从 CHANGELOG.md 提取指定版本的发布说明，供 Release 正文使用。
#
# 用法：
#   bash scripts/release-notes.sh 0.5.0            # 输出到 stdout
#   bash scripts/release-notes.sh v0.5.0 notes.md   # 写入文件
#
# 行为与约束：
#   - 版本号允许带前缀 v（tag 形式），匹配 `## [0.5.0]` 或 `## 0.5.0`；
#   - 只取该版本小节，止于下一个 `## ` 标题，不会把「未发布」内容带进 Release；
#   - 去掉链接引用定义（`[0.5.0]: https://…`）与尾部空行；
#   - **找不到对应小节时退出码 1**：发布流程据此回退到通用文案，而不是发一份空的
#     更新说明（Release notes 与 CHANGELOG 必须是同一份事实）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${1:-}"
OUT="${2:-}"
CHANGELOG="${CHANGELOG:-$ROOT/CHANGELOG.md}"

die() { echo "[release-notes] 错误: $*" >&2; exit 1; }

[[ -n "$VERSION" ]] || die "用法: $0 <版本号> [输出文件]"
[[ -f "$CHANGELOG" ]] || die "找不到变更日志: $CHANGELOG"

# 去掉 tag 前缀 v，容忍 v0.5.0 / 0.5.0 两种写法
VERSION="${VERSION#v}"

BODY="$(awk -v ver="$VERSION" '
  BEGIN { inside = 0 }
  /^## / {
    if (inside) exit
    if ($0 ~ "^## \\[" ver "\\]" || $0 ~ "^## " ver "([[:space:]]|$)") { inside = 1 }
    next
  }
  inside { print }
' "$CHANGELOG")"

# 链接引用定义不属于正文（Keep a Changelog 把它们放在文件末尾，个别项目会紧贴小节）
BODY="$(printf '%s\n' "$BODY" | grep -v -E '^\[[^]]+\]:[[:space:]]*https?://' || true)"
# 去尾部空行（保留内部空行）
BODY="$(printf '%s\n' "$BODY" | sed -e 's/[[:space:]]*$//' | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"

if [[ -z "${BODY//[[:space:]]/}" ]]; then
  die "CHANGELOG 中没有 $VERSION 的小节（请先补写发布说明）"
fi

if [[ -n "$OUT" ]]; then
  printf '%s\n' "$BODY" > "$OUT"
  echo "[release-notes] 已写入 $OUT（$VERSION）" >&2
else
  printf '%s\n' "$BODY"
fi
