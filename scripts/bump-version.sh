#!/usr/bin/env bash
# 一键同步发布版本号：版本号分散在 5 个文件，手改必漏。
#
# 用法：
#   bash scripts/bump-version.sh 0.5.0            # 执行
#   bash scripts/bump-version.sh 0.5.0 --dry-run  # 预览要改什么，不写盘
#
# 覆盖位置：
#   Cargo.toml            [workspace.package] version（rb-core / rb-app 均继承）
#   Cargo.lock            workspace 成员版本（cargo update -w 同步）
#   crates/rb-app/tauri.conf.json   应用版本（决定安装包文件名与更新清单）
#   ui/package.json + ui/package-lock.json   前端工程版本（lock 同步前两处根版本）
#   CITATION.cff          引用元数据版本
#
# 约束：
#   - package-lock.json 中存在依赖包恰好也叫 0.1.0 的情况，因此**只改前两处**
#     根版本（顶层 + packages[""]），绝不全局替换；
#   - 版本号变更不在这里打 tag、不改 CHANGELOG——那是发布检查单的后续步骤
#     （见 docs/distribution.md）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NEW="${1:-}"
DRY="${2:-}"

die() { echo "[bump-version] 错误: $*" >&2; exit 1; }
info() { echo "[bump-version] $*"; }

[[ -n "$NEW" ]] || die "用法: $0 <版本号> [--dry-run]"
[[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.]+)?$ ]] || die "版本号格式不合法: ${NEW}（示例 0.5.0 / 1.0.0-rc.1）"

# 从 Cargo.toml 读当前版本（唯一事实源），避免猜测
OLD="$(grep -m1 '^version = "' "$ROOT/Cargo.toml" | sed 's/^version = "\(.*\)"$/\1/')"
[[ -n "$OLD" ]] || die "无法从 Cargo.toml 读取当前版本"
if [[ "$OLD" == "$NEW" ]]; then
  die "版本号已是 ${NEW}（如需重打请先回退或换号）"
fi

# 定点替换：按行首模式定位，只替换前 limit 处；check 非空时只校验不写盘
bump() {
  local file="$1" pattern="$2" old="$3" new="$4" limit="$5" check="${6:-}"
  DRY_MODE="${check:+1}${DRY}" python3 - "$ROOT/$file" "$pattern" "$old" "$new" "$limit" <<'PY'
import os, re, sys

path, pattern, old, new, limit = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5])
with open(path, encoding="utf-8") as f:
    lines = f.readlines()

done = 0
for i, line in enumerate(lines):
    if done >= limit:
        break
    if re.search(pattern, line) and old in line:
        lines[i] = line.replace(old, new, 1)
        done += 1

if done == 0:
    sys.exit(f"[bump-version] 错误: {path} 中没有匹配 {pattern} 的行")

if os.environ.get("DRY_MODE"):
    print(f"  {os.path.relpath(path)}: {done} 处 {old} -> {new}（dry-run 未写入）")
else:
    with open(path, "w", encoding="utf-8") as f:
        f.writelines(lines)
    print(f"  {os.path.relpath(path)}: {done} 处 {old} -> {new}")
PY
}

# 五个目标文件的版本写点（package-lock 根版本在顶层与 packages[""] 两处；
# 依赖树里可能有同名版本，因此只替换前两处，且锚定旧版本号原文）
apply_all() {
  local check="${1:-}"
  bump "Cargo.toml"                    '^version = "'        "version = \"$OLD\"" "version = \"$NEW\"" 1 "$check"
  bump "crates/rb-app/tauri.conf.json" '"version": "'        "\"version\": \"$OLD\"" "\"version\": \"$NEW\"" 1 "$check"
  bump "ui/package.json"               '"version": "'        "\"version\": \"$OLD\"" "\"version\": \"$NEW\"" 1 "$check"
  bump "ui/package-lock.json"          '"version": "'        "\"version\": \"$OLD\"" "\"version\": \"$NEW\"" 2 "$check"
  bump "CITATION.cff"                  '^version: "'         "version: \"$OLD\"" "version: \"$NEW\"" 1 "$check"
}

info "$OLD -> $NEW"
# 两阶段：先对全部目标做只读校验（任一文件不匹配立即退出，此时尚未写盘），
# 再统一写入——避免中途失败留下「一半已改、一半未改」的仓库状态。
apply_all check
apply_all

if [[ "$DRY" == "--dry-run" ]]; then
  info "dry-run 结束，未写盘"
  exit 0
fi

# Cargo.lock 里 workspace 成员的版本由 cargo 负责同步（不碰第三方依赖）
if command -v cargo >/dev/null 2>&1; then
  (cd "$ROOT" && cargo update -w --quiet)
  info "Cargo.lock 已通过 cargo update -w 同步"
else
  die "未找到 cargo：请先安装 Rust 工具链，或手动执行 cargo update -w"
fi

if ! grep -q "^## \[$NEW\]" "$ROOT/CHANGELOG.md"; then
  info "提醒：CHANGELOG.md 尚无「## [$NEW]」小节——发布正文的来源是它（scripts/release-notes.sh）"
fi
# 文档漂移检查：这两处不会随 bump 自动更新，发版前人工核对
if grep -q "compare/v[0-9.]*\.\.\.HEAD" "$ROOT/CHANGELOG.md"; then
  info "提醒：CHANGELOG.md 底部的 compare 链接基线仍是旧 tag，发版时请更新为上一版本"
fi
if grep -qE "v0\.[0-9]+\.x" "$ROOT/README.md" "$ROOT/README_EN.md"; then
  info "提醒：README 分节标题仍带具体版本号（如 v0.1.x），已过时请移除版本字样"
fi

info "完成。后续步骤见 docs/distribution.md「发布检查单」"
