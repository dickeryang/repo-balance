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
{"repo":"/path/to/repo","totalScore":85,"findingCount":3,"critical":1,"warning":2,"info":0,"scope":null,"findings":[{"id":"big-files.history:a.bin","severity":"critical","title":"历史中存在大文件..."}]}
```

可在 CI 中解析 JSON 后自定义质量门禁（如总分 < 70 则失败）。
