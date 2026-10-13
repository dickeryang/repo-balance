# CI 集成

仓衡 `rb` CLI 支持在 CI 流水线中自动运行体检，按退出码反映结果。

## CLI 用法

```bash
rb scan <path> [--ci|--json|--quiet]
```

### 选项

| 选项 | 说明 |
|------|------|
| `--ci` | CI 模式：简洁输出，适合日志展示 |
| `--json` | JSON 模式：单行 JSON 输出，适合机器解析 |
| `--quiet` | 静默模式：不输出详情，仅按退出码反映结果 |

### 退出码

| 码 | 含义 |
|----|------|
| 0 | 无 Critical 问题 |
| 1 | 运行错误（仓库无法打开等） |
| 2 | 存在 Critical 问题（CI 应标记失败） |

## GitHub Actions

参考 `.github/workflows/repo-balance.yml`，核心步骤（在**仓库根目录**执行，产物落在 `target/release/`）：

```yaml
- name: 构建 rb CLI
  run: cargo build --release -p rb-core

- name: 运行仓衡体检
  run: target/release/rb scan . --ci
```

流水线在 push / PR 时触发，存在 Critical 问题时自动失败。

## 本仓库自身的流水线

本仓库用多条职责正交的流水线，可作接入参考：

| 流水线 | 文件 | 职责 |
|---|---|---|
| 质量门禁 | `.github/workflows/ci.yml` | 格式 / clippy（零 warning）/ 单元测试 / 前端 lint 与构建；依赖许可证与漏洞审计（`deny.toml` + cargo-deny）；rb-core 覆盖率摘要 |
| 自身体检 | `.github/workflows/repo-balance.yml` | `rb scan . --ci` 扫描本仓库，存在 Critical 结论即失败 |
| 桌面打包 | `.github/workflows/build-desktop.yml` | tag 触发，Windows / macOS / Linux 三平台安装包 |
| AtomGit 流水线 | `ci/atomgit-pipeline.yml` | AtomGit 侧的质量门禁（rb-core）+ 自身体检 |

> 格式门禁为 `cargo fmt --all -- --check`，本地提交前请先执行；完整贡献流程见 `CONTRIBUTING.md`。

## AtomGit 流水线

参考 `ci/atomgit-pipeline.yml`，在 AtomGit 仓库「流水线」设置中引用即可。
该文件分「质量门禁」（格式 / clippy / 测试，覆盖 rb-core）与「体检」两段；
`rb-app` 依赖 webkit2gtk 等桌面系统库，不在容器内构建。

## 通用 Shell 示例

适用于任意 CI 系统（Jenkins / Drone / Gitea Actions 等）：

```bash
#!/bin/bash
set -e

# 构建 CLI
cargo build --release -p rb-core

# 运行体检（--ci 模式，Critical 时退出码 2）
./target/release/rb scan . --ci

# 捕获退出码
code=$?
if [ $code -eq 2 ]; then
  echo "体检未通过：存在 Critical 问题"
  exit 1
elif [ $code -ne 0 ]; then
  echo "体检运行出错"
  exit 1
fi

echo "体检通过"
```

## JSON 输出示例

```bash
rb scan . --json
```

```json
{"repo":"/path/to/repo","totalScore":85,"findingCount":3,"critical":1,"warning":2,"info":0,"findings":[{"id":"big-files.history:a.bin","severity":"critical","title":"历史中存在大文件..."}]}
```

可在 CI 中解析 JSON 后自定义质量门禁（如总分 < 70 则失败）。
