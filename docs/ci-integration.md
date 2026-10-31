# CI 集成

仓衡 `rb` CLI 支持在 CI 流水线中自动运行体检，按退出码反映结果。

## CLI 用法

```bash
rb scan <path> [--ci|--json|--quiet] [--max-commits N] [--since-days N] [--vuln-db FILE] [--rules FILE]
```

### 选项

| 选项 | 说明 |
|------|------|
| `--ci` | CI 模式：简洁输出，适合日志展示 |
| `--json` | JSON 模式：单行 JSON 输出，适合机器解析 |
| `--quiet` | 静默模式：不输出详情，仅按退出码反映结果 |
| `--max-commits N` | 只遍历最新 N 个提交（N > 0），大仓库提速 |
| `--since-days N` | 只遍历最近 N 天的提交（N > 0），大仓库提速 |
| `--vuln-db FILE` | 离线漏洞库索引（JSONL）；不指定则不做依赖漏洞匹配 |
| `--rules FILE` | 自定义规则（JSON 数组）；不指定则不启用 `custom-rules` 检查器 |

`--max-commits` 与 `--since-days` 可叠加，取交集。历史遍历与提交数近似线性，
超大仓库建议限制范围以控制流水线时长：

```bash
# 只看最近半年的历史（PR 流水线常用）
rb scan . --ci --since-days 180

# 只看最近 2000 个提交
rb scan . --ci --max-commits 2000
```

> **受限扫描会如实标注**：启用任一限制后，`--ci` 输出会多一行「范围」说明，
> `--json` 输出会多一个 `scope` 字段（全量扫描为 `null`），导出的报告同样带该说明。
> 请勿把受限结论当作全量结论解读。

## 依赖漏洞检测（离线漏洞库）

漏洞库是**离线索引文件**，不在扫描时联网。CI 用法：

```bash
# 1) 生成索引（需网络，开发者/运维手工执行一次，产物可提交或缓存）
bash scripts/build-vuln-index.sh ./vuln-index.jsonl

# 2) 扫描时指定
rb scan . --ci --vuln-db ./vuln-index.jsonl
```

- 支持的依赖文件：`Cargo.lock` / `Cargo.toml` / `package-lock.json` / `package.json` /
  `yarn.lock` / `go.mod` / `requirements.txt` / `pom.xml`；
- 未指定 `--vuln-db` 时依赖维度仅做结构检查，**行为与不接入漏洞库时完全一致**（不报错、不扣分）；
- 建议在 CI 中缓存索引文件并定期重新生成（漏洞库会持续更新）。

## 自定义规则

把团队规约（禁用 API、遗留标记等）作为 JSON 规则随仓库一起提交，CI 即可零配置复现：

```bash
rb scan . --ci --rules ./ci/repobalance-rules.json
```

- 规则文件不存在 / JSON 非法 / 正则无法编译一律**快速失败**（退出码 1），不做静默降级；
- 格式与行为口径（命中聚合、样本掩码、单规则 50 文件上限）见 [`custom-rules.md`](custom-rules.md)。

## 忽略规则校验

`.repobalance-ignore` 随仓库提交，但「写了却不会生效」的规则既不会报错、也不会生效
（反斜杠路径、空模式、重复规则）——CI 里加一行就能在合并前拦住：

```bash
rb ignore .
```

- **只读**校验，不写任何文件；打印用户规则条数 + 内置默认条数；
- 存在可疑规则时逐条列出「第 N 行 `模式`：原因」并以退出码 1 结束；仓库未配置忽略规则时退出码 0；
- 与 GUI 设置页「忽略规则」的试算同一口径（内置默认 + 用户规则，最后匹配胜出），
  语法与完整口径见 [`ignore-rules.md`](ignore-rules.md)。

### 退出码

| 码 | 含义 |
|----|------|
| 0 | 无 Critical 问题 |
| 1 | 运行错误（仓库无法打开等） |
| 2 | 存在 Critical 问题（CI 应标记失败） |

> `rb ignore` 独立口径：0 = 检查通过（含仓库未配置忽略规则），1 = 存在可疑规则 / 读取失败。

## 官方 GitHub Action（一行接入）

本仓库自带 composite action：下载 Release 里的预编译 CLI → 本地体检 → 出报告，
存在 Critical 结论时任务失败，可选在 PR 上原位更新评论：

```yaml
# .github/workflows/repobalance.yml
name: 仓衡体检
on: [push, pull_request]
permissions:
  contents: read
  pull-requests: write   # 仅 comment: true 时需要
jobs:
  checkup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dickeryang/repo-balance@v0
        with:
          path: .
          args: --max-commits 500      # 大仓库建议限范围
          comment: true                # PR 上更新体检评论
```

### 输入

| 输入 | 默认 | 说明 |
|---|---|---|
| `path` | `.` | 要体检的仓库或目录 |
| `args` | 空 | 附加 CLI 参数（`--max-commits` / `--since-days` / `--rules` / `--vuln-db`） |
| `cli-version` | `latest` | CLI 来源 Release tag；建议 pin 到具体版本保证可复现 |
| `fail-on` | `critical` | `critical` = 存在 Critical 失败；`never` = 只报告不拦截 |
| `comment` | `false` | 在 PR 上发表 / 原位更新体检评论（仅 pull_request 事件） |
| `report-artifact` | `repo-balance-report` | JSON 报告 artifact 名；留空不上传 |
| `token` | `github.token` | 下载 CLI 与评论用；fork PR 场景建议传 PAT |

### 输出

`total-score`（总分，全部 N/A 时为空）、`critical-count`、`conclusion`（`pass` / `critical`），
可用于后续步骤自定义门禁（如总分低于阈值时告警）。

### 行为与边界

- **单次扫描两用**：`--json` 模式在存在 Critical 时同样以退出码 2 结束，报告与门禁
  不需要扫两遍；退出码口径见上文「退出码」；
- **评论原位更新**：评论带稳定标记，重复运行只编辑同一条，不会刷屏；fork PR 的
  默认 token 只读，评论 403 时降级为警告（体检结论不受影响）；
- **CLI 产物**：action 下载 Release 资产 `repo-balance-cli-<platform>.tar.gz`，自
  **包含该资产的 Release 起**可用（此前版本的 Release 没有 CLI 产物，会 404）；
  Linux 产物在 ubuntu-22.04 构建，要求 glibc ≥ 2.35；macOS 为 universal 双架构；
- 引用建议 pin 到 tag（如 `@v0.5.0`）或 commit SHA，避免 master 变更影响流水线。

## 自建步骤（源码编译）

不想用 Action 或在非 GitHub 平台，可参考 `.github/workflows/repo-balance.yml` 的核心步骤
（在**仓库根目录**执行，产物落在 `target/release/`）：

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
{"repo":"/path/to/repo","totalScore":85,"findingCount":3,"critical":1,"warning":2,"info":0,"scope":null,"findings":[{"id":"big-files.history:a.bin","severity":"critical","title":"历史中存在大文件..."}]}
```

可在 CI 中解析 JSON 后自定义质量门禁（如总分 < 70 则失败）。
