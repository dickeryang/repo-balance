# 仓衡 RepoBalance 功能路线图

> 本文档是仓衡的权威规划文档，与 README「规划」章节保持一致。
> 规则：每个版本发布时同步更新勾选状态；**未完成的功能不得在 README 中标记为已完成**。
> 最近更新：2026-10-12

---

## 〇、现状盘点（2026-10-12）

### 已落地

| 能力 | 状态 | 说明 |
|---|---|---|
| Checker 插件契约 | ✅ | trait + 注册表 + plan/merge 两阶段编排，`rb-core` 零 tauri 依赖 |
| 五个内置检查器 | ✅ | 大文件 / 敏感信息 / 僵尸分支 / 提交习惯 / 依赖健康，五维均有真实结论 |
| 敏感信息检查器 | ✅ | 私钥头、AWS Access Key、GitHub/GitLab token、`.env` 赋值、高熵密钥（正则 + 香农熵双通道），evidence 构造时掩码 + 停用词白名单 |
| 僵尸分支检查器 | ✅ | 已合并未删除、超 N 天未活跃（默认 90 天可配）、无远端追踪；`BranchGraph` 切片 |
| 提交习惯检查器 | ✅ | 超大提交（>50 Warning / >200 Critical）、低质信息占比、深夜比例（按提交时区） |
| 依赖健康检查器（轻量） | ✅ | 锁定文件缺失/未提交/被删、同生态多管理器混用；`DependencyFiles` 切片 |
| 评分口径 | ✅ | 未覆盖维度 N/A（非 100），总分 = 已覆盖维度平均 |
| 文件内容切片 | ✅ | `FileContents` 携带受预算约束的文本内容（单文件 1 MiB / 全仓 32 MiB / 二进制跳过） |
| 忽略规则 | ✅ | `.repobalance-ignore` + 否定规则；内置默认忽略 `target/`、`node_modules/` 等构建目录（可 `!` 否定） |
| 扫描编排 | ✅ | 进度事件（snapshot/workdir/history/check/done，check 逐检查器带 finding 数）、取消、部分报告 |
| 报告导出 | ✅ | JSON / Markdown / HTML（自包含，内联 SVG 雷达图，evidence 掩码） |
| 扫描历史与对比 | ✅ | localStorage 持久化，新增/消除问题项对比 |
| 国际化 | ✅ | 中英切换（自写字典） |
| CLI | ✅ | `rb scan <path> [--ci\|--json\|--quiet]` |
| CI | ✅ | GitHub Actions：自体检 + Release 跨平台打包 |
| 安全设计 | ✅ | CSP 最小集、权限最小集、纯本地零网络、evidence 掩码 |
| 贡献者基础设施 | ✅ | CONTRIBUTING / CODE_OF_CONDUCT / SECURITY / CHANGELOG / issue 模板（bug+feature）/ PR 模板 / .editorconfig / .gitattributes |
| 演示可复现 | ✅ | `scripts/build-demo-fixture.sh` 在全新克隆上自动初始化演示仓库（幂等，四类合成数据） |
| 双语文档 | ✅ | `README.md` + `README_EN.md` 互链 |
| AtomGit 露出 | ✅ | README 顶部 G-Star+Star+Fork+Download 徽章、托管与同步章节、AtomGit logo |
| 双端同步 | ✅ | 同一提交推送 AtomGit + GitHub（G-Star 要求同步不超过 1 个月） |

### 关键缺口

1. 历史数据存 localStorage：约 5MB 上限、清除缓存即丢失、无法导出。
2. PDF 导出未实现。

---

## 一、短期（0~2 个月）

**阶段目标：修复「四维假满分」诚信问题，通过 GitCode G-Star 认证。**

### Sprint 1 · v0.2.0（第 1~3 周）

- [x] **敏感信息检查器**（Security 维度）
  - 基于已有 `FileContents` 切片，不引入新切片类型（切片载荷扩展为携带文本内容）；
  - 内置规则：私钥头（`-----BEGIN … PRIVATE KEY-----`）、AWS Access Key、GitHub / GitLab token、`.env` 赋值模式；
  - 高熵字符串检测作为第二通道（香农熵 ≥ 4.2、长度 ≥ 20、键名含 key/token/secret 等），降低纯正则的漏报；
  - evidence 沿用既有掩码机制，构造时即掩码；停用词白名单跳过文档示例值。
- [x] **僵尸分支检查器**（Branches 维度）
  - `SliceData` 增加 `BranchGraph` 变体，引擎采集分支拓扑（tip 日期 / 是否已合并 / 有无远端追踪）；
  - 规则：已合并未删除、超 90 天未活跃（`zombie_branch_days` 可配）、无远端追踪的本地分支。
- [x] **评分口径修复**
  - 无检查器覆盖的维度显示 `N/A` 而非 100；
  - 总分改为「已覆盖维度平均」，避免新增检查器后分数反升的误导。
- [x] **README 规划区修正**：未实现的功能（PDF 导出等）从「已完成」移至本文档对应位置。
- [x] **贡献者基础设施四件套**
  - `CONTRIBUTING.md`（含代码纪律、新增检查器三步法、质量门禁）、
    issue 模板（bug / feature）、PR 模板；
  - README 增加「参与贡献」章节。
- [x] **G-Star 前置准备（代码侧）**
  - README 顶部加 AtomGit G-Star / Star / Fork / Download 官方徽章（硬性要求：项目内有 AtomGit 露出）；
  - README 增加「托管与同步」章节，写明国内 AtomGit 官方托管地址与双端同步约定；
  - `docs/AtomGit-logo.svg`（官方指南提供）随 README 展示。
- [ ] **提交 G-Star 认证申请**（https://atomgit.com/g-star/apply）
  - 需人工在 AtomGit 完成：仓库简介与头像、Topics、确认 License 被识别为 Apache-2.0、
    可选开启仓库自动双向同步；评审周期 1-2 个工作日。

### Sprint 2 · v0.3.0（第 4~6 周）

- [x] **提交习惯检查器**（History 维度）——已提前至本轮落地
  - 基于 `snapshot.recent_commits` + `FullHistory` 切片聚合：超大提交（>50 文件 Warning / >200 Critical）、`wip` / `fix typo` 类低质信息占比（样本 ≥5 且 >30%）、深夜提交比例（按提交时区换算）。
- [x] **依赖健康检查器 · 轻量版**（Deps 维度）——已提前至本轮落地
  - 仅结构检查：锁定文件缺失 / 未提交 / 被删（manifest 与 lock 不同步）、同生态多包管理器混用；
  - 版本解析与漏洞库见中期任务 1。
- [ ] **历史持久化迁移**
  - localStorage → 应用数据目录 JSON 文件（capabilities 增加 fs 最小权限）；
  - 收益：突破 5MB 上限、不随清缓存丢失、支持导出；为中期趋势分析铺路。
- [ ] **大仓库性能**
  - 历史遍历增加 `--max-commits` / 时间窗限制；
  - 扫描前展示规模预估；目标：超大规模仓库可中断、进度可感知。

### 短期验收标准

- [x] `cargo test --workspace` 全绿；每个新检查器覆盖「命中 / 不命中 / 边界阈值 / 忽略规则」四类用例（沿用现有中文测试名 + `TestRepo` 范式）；
- [x] 对真实中型仓库扫描，五个维度均产出真实结论（本仓库自体验证：五维全部计分，无占位假分）；
- [ ] G-Star 认证通过。

---

## 二、中期（2~6 个月）

**阶段目标：从「本地体检工具」演进为「团队仓库治理工具」。**

- [ ] **依赖漏洞数据库**（Deps 维度升级）
  - 解析 Cargo.toml / package.json / pom.xml / go.mod 的版本清单；
  - 集成 **OSV 离线库**（Apache-2.0，JSON 格式可随包分发）；
  - 形成差异化卖点：一个工具检测全栈依赖漏洞（单语言工具如 `cargo audit` 只能覆盖一种生态）。
- [ ] **配置驱动的自定义规则**
  - 用户以 JSON 定义规则（正则 + 路径 glob + 严重度 + 话术），GUI 设置页管理；
  - 作为插件机制的轻量替代先行落地，验证真实需求后再投入插件系统。
- [ ] **远程仓库体检**
  - 输入 URL → `--depth 1` 浅克隆到临时目录 → 本地扫描 → 自动清理；
  - 复用 tempfile 依赖，保持纯本地语义。
- [ ] **多仓库批量体检**
  - 仓库队列扫描 + 汇总对比看板；
  - 面向团队仓库治理场景，是认证后拉新与传播的主打功能。
- [ ] **健康度趋势曲线**
  - 基于已迁本地文件的历史数据，按仓库绘制五维趋势图；复用 RadarChart 几何。
- [ ] **设置页独立化**
  - 阈值 / 检查器开关 / 忽略规则编辑从「连接页」拆出为独立设置页。
- [ ] **分发渠道扩展**
  - `tauri-plugin-updater` 自动更新；
  - Release notes 规范化；
  - Homebrew / Scoop / Winget 打包脚本。
- [ ] **前端测试**
  - UI 侧引入 Vitest，优先覆盖 store 的配置与历史逻辑。

### 中期验收标准

- v0.5.0 发布，具备 OSV 漏洞检测、批量体检、趋势图三项核心能力；
- AtomGit star 达到三位数（批量体检面向团队场景，易形成传播）。

---

## 三、长期（6~12+ 个月）

**阶段目标：建立生态位，从应用走向平台。**

- [ ] **检查器插件机制（WASM 沙箱）**
  - 基于 wasmtime / extism，允许第三方以任意语言编写检查器；
  - **优先 WASM 而非动态库（dylib）**：沙箱隔离、跨平台、无 ABI 兼容负担。
- [ ] **官方 GitHub Action**
  - CLI 封装为 docker action，CI 中一行 `uses:` 触发体检并评论 PR；
  - 配套 GitLab CI 模板与 pre-commit hook。
- [ ] **团队报告协作（Report as Code）**
  - 报告存入团队 Git 仓库，整改项经 PR 评审闭环；
  - 可选加密分享链接作为云增值，默认保持纯本地。
- [ ] **rb-core 跨语言嵌入**
  - 暴露 C ABI / wasm 包，供 IDE 插件、DevOps 平台嵌入检查引擎——从应用变为库。
- [ ] **仓库基准分**
  - opt-in 匿名化基准数据集：「你的仓库健康度在同类项目中排在什么位置」；
  - 必须明示 opt-in，与纯本地定位做显式区隔。

---

## 四、执行原则

1. **架构纪律不可破**：`rb-core` 零 tauri 依赖、git2 类型不逃逸 git 模块、公共类型 derive 序列化、非测试代码禁 `unwrap()` / `expect()`。这是 CLI / CI / 插件化的根基。
2. **新检查器必须带测试**：每个检查器至少覆盖「命中 / 不命中 / 边界阈值 / 忽略规则」四类用例。
3. **文档与代码同步**：README 与本文档不得超前于实现；每次版本发布同步勾选。
4. **发布节奏**：每 4~6 周一个 minor 版本，Release notes + 技术博客同步发布，为 AtomGit / GitHub 导流。
