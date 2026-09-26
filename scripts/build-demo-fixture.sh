#!/usr/bin/env bash
# 仓衡演示仓库夹具构建脚本（幂等，playbook 2.2）。
# 用法：在主仓库根目录执行 bash scripts/build-demo-fixture.sh
# 行为：为 demo-repo 补齐三类合成数据（leaked.txt / big.bin / old-branch），
#       已满足的步骤自动跳过；不重写、删除或 Amend 既有提交。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEMO="$ROOT/demo-repo"

die() { echo "[夹具] 错误: $*" >&2; exit 1; }
info() { echo "[夹具] $*"; }

# 步骤 0：环境自检（本脚本仅在 demo-repo 独立 git 仓库内操作，与主仓库
# 工作区变更完全隔离；主仓库脏变更仅警告不中止，避免编码任务进行态误伤）
cd "$ROOT"
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  info "警告：主仓库存在未提交变更（本脚本不触碰主仓库已跟踪文件，继续执行）"
fi
if [ ! -d "$DEMO/.git" ]; then
  die "demo-repo 不存在或不是 git 仓库，请先初始化"
fi
cd "$DEMO"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$BRANCH" != "main" ]; then
  die "当前分支为 $BRANCH，预期 main；请先切换"
fi
if [ -z "$(git config user.name || true)" ]; then
  git config user.name "repo-balance-demo"
  git config user.email "demo@repo-balance.local"
  info "已配置仓库级演示身份"
fi

# 步骤 1：leaked.txt —— DEMO KEY 占位块（恰三行，SAMPLE-NOT-A-REAL-KEY 红线）
EXPECTED_LEAKED='-----BEGIN DEMO KEY BLOCK-----
SAMPLE-NOT-A-REAL-KEY
-----END DEMO KEY BLOCK-----'
if git cat-file -e HEAD:leaked.txt 2>/dev/null; then
  info "跳过：leaked.txt 已在 main 历史"
elif [ -f leaked.txt ]; then
  die "leaked.txt 在工作区但未提交，形态不符，请人工确认"
else
  printf '%s\n' "$EXPECTED_LEAKED" > leaked.txt
  git add leaked.txt
  git commit -q -m "chore: add leaked placeholder"
  info "已写入 leaked.txt 并提交"
fi

# 步骤 2：big.bin —— 恰 5242880 字节大文件（提交后仍留工作区 → Warning 演示形态）
if git cat-file -e HEAD:big.bin 2>/dev/null; then
  info "跳过：big.bin 已在 main 历史"
  if [ -f big.bin ]; then
    SIZE="$(wc -c < big.bin | tr -d ' ')"
    if [ "$SIZE" != "5242880" ]; then
      die "big.bin 字节数为 $SIZE，预期 5242880，形态不符"
    fi
  else
    info "提示：big.bin 仅存在于历史、工作区已移除（Critical 演示形态）"
  fi
elif [ -f big.bin ]; then
  die "big.bin 在工作区但未提交，形态不符，请人工确认"
else
  head -c 5242880 /dev/zero > big.bin
  git add big.bin
  git commit -q -m "chore: big file"
  info "已生成 big.bin（5242880 字节）并提交"
fi

# 步骤 3：old-branch —— 僵尸分支（相对 main 存在未合并提交）
if git show-ref --verify --quiet refs/heads/old-branch; then
  info "跳过：old-branch 已存在"
else
  git checkout -q -b old-branch
  printf 'stale\n' > s.txt
  git add s.txt
  git commit -q -m "wip"
  git checkout -q main
  info "已创建 old-branch（含未合并提交 wip）"
fi

# 步骤 4：最终形态报告
info "===== 形态报告 ====="
info "分支: $(git branch --format='%(refname:short)' | tr '\n' ' ')"
info "main 提交链:"
git log --oneline main | sed 's/^/  /'
info "old-branch 相对 main 的未合并提交:"
git log --oneline main..old-branch | sed 's/^/  /' || true
for f in leaked.txt big.bin; do
  if git cat-file -e "main:$f" 2>/dev/null; then
    info "$f: 已提交入 main 历史 ✓"
  else
    info "$f: 未在 main 历史 ✗"
  fi
done
if git cat-file -e old-branch:s.txt 2>/dev/null; then
  info "s.txt（old-branch）: 已提交 ✓"
else
  info "s.txt（old-branch）: 缺失 ✗"
fi
info "===== 夹具就绪 ====="