# 仓衡 RepoBalance 功能路线图

> 本文档是仓衡的权威规划文档，与 README「规划」章节保持一致。
> 规则：每个版本发布时同步更新勾选状态；**未完成的功能不得在 README 中标记为已完成**。
> 最近更新：2026-10-30

---

## 〇、现状盘点（2026-10-30）

### 已落地

| 能力 | 状态 | 说明 |
|---|---|---|
| Checker 插件契约 | ✅ | trait + 注册表 + plan/merge 两阶段编排，`rb-core` 零 tauri 依赖 |
| 六个内置检查器 | ✅ | 大文件 / 敏感信息 / 僵尸分支 / 提交习惯 / 依赖健康 / 合并冲突残留，五维均有真实结论 |
| 敏感信息检查器 | ✅ | 私钥头、AWS Access Key、GitHub/GitLab token、`.env` 赋值、高熵密钥（正则 + 香农熵双通道），evidence 构造时掩码 + 停用词白名单 |
| 僵尸分支检查器 | ✅ | 已合并未删除、超 N 天未活跃（默认 90 天可配）、无远端追踪；`BranchGraph` 切片 |
| 提交习惯检查器 | ✅ | 超大提交（>50 Warning / >200 Critical）、低质信息占比、深夜比例（按提交时区） |
| 合并冲突残留检查器 | ✅ | 工作区与历史提交中残留的冲突标记；三重防误报（三元组齐备 / 文档示例停用词 / 忽略规则），历史口径不强制改史 |
| 自定义规则检查器 | ✅ | JSON 定义「正则 + 路径 glob + 严重度 + 维度 + 话术」，GUI 表单管理与导入导出、CLI `--rules`；按「规则 × 文件」聚合、样本掩码、单规则 50 文件上限；无规则不注册（不产生假满分） |
| 依赖健康检查器 | ✅ | 锁定文件缺失/未提交/被删、同生态多管理器混用；接入离线漏洞库后追加已知漏洞结论（切片 `DependencyFiles` + `DependencyFileContents`） |
| 评分口径 | ✅ | 未覆盖维度 N/A（非 100），总分 = 已覆盖维度平均 |
| 文件内容切片 | ✅ | `FileContents` 携带受预算约束的文本内容（单文件 1 MiB / 全仓 32 MiB / 二进制跳过） |
| 忽略规则 | ✅ | `.repobalance-ignore` + 否定规则；内置默认忽略 `target/`、`node_modules/` 等构建目录（可 `!` 否定） |
| 忽略规则编辑器 | ✅ | 设置页选定仓库后直接读写 `.repobalance-ignore`（原子写）；「试算」不落盘求值（口径与扫描同源）并给出被忽略文件数 / 占比 / 体积与样本；静态检查「写了却不会生效」的规则；CLI `rb ignore <path>` 同口径只读校验；见 [`ignore-rules.md`](ignore-rules.md) |
| 扫描编排 | ✅ | 进度事件（snapshot/workdir/history/check/done，check 逐检查器带 finding 数）、取消、部分报告 |
| 报告导出 | ✅ | JSON / Markdown / HTML（自包含，内联 SVG 雷达图，evidence 掩码） |
| 大仓库性能闸门 | ✅ | 历史遍历支持「最多 N 个提交」/「最近 N 天」（CLI `--max-commits` / `--since-days`，GUI「历史范围」分组）；受限扫描在 CLI / 报告页 / 导出中如实标注 |
| 扫描规模预估 | ✅ | `scan_estimate` 返回提交数/文件数/体积（精确）+ 历史对象数与耗时（采样外推）；连接页四档展示并给出建议限制 |
| 扫描历史与对比 | ✅ | 应用数据目录 `scan-history.json` 持久化（原子写入，旧 localStorage 一次性迁移），新增/消除问题项对比 |
| 多仓库批量体检 | ✅ | 多选目录 → 串行队列 → 汇总对比看板（`BatchView`，视图 5）：整批进度、每仓状态 / 总分 / 问题数 / 严重数 / 用时、按总分排序对比；单仓失败不中断整批，取消可中止并标记剩余；完成仓库写入扫描历史，可跳转报告页 |
| 健康度趋势分析 | ✅ | 历史页按仓库绘制「总分 + 五维」折线与时间轴（`TrendChart`）：N/A 维度断线不插值、图例可隐藏曲线、tooltip 标注单点时间与取值；数据源为本地扫描历史（全局最多 20 条，页面如实标注） |
| 国际化 | ✅ | 中英切换（自写字典） |
| CLI | ✅ | `rb scan <path> [--ci\|--json\|--quiet] [--max-commits N] [--since-days N] [--vuln-db FILE] [--rules FILE]` |
| 依赖漏洞检测 | ✅ | 离线 OSV 索引（JSONL）+ 8 类依赖文件解析 + 自写跨生态版本比较；未接入索引时退回结构检查 |
| CI | ✅ | 质量门禁（fmt / clippy -D warnings / test / 前端 lint+build）+ 自体检 + Release 跨平台打包（含 Linux） |
| 供应链治理 | ✅ | `deny.toml`（许可证白名单 / 漏洞 / 来源，cargo-deny）、Dependabot 周更、`.github/CODEOWNERS` |
| 覆盖率 | ✅ | rb-core 覆盖率摘要纳入 CI（cargo-llvm-cov） |
| MSRV | ✅ | 声明 `rust-version = 1.88`（依赖树实测下界），CI 与实际一致 |
| 安全设计 | ✅ | CSP 最小集、权限最小集、默认零网络（唯一出站为手动的更新检查）、更新包 minisign 验签、evidence 掩码 |
| 贡献者基础设施 | ✅ | CONTRIBUTING / CODE_OF_CONDUCT / SECURITY / CHANGELOG / SUPPORT / GOVERNANCE / AUTHORS / CITATION.cff / issue 模板（bug+feature）/ PR 模板 / .editorconfig / .gitattributes |
| 双语文档 | ✅ | README / CONTRIBUTING / SECURITY / CODE_OF_CONDUCT 四份文档中英互链（CHANGELOG 保持单语，避免双份同步负担） |
| 安装体验说明 | ✅ | README 载明发布包未签名及 macOS/Windows 放行方式（含命令），并指向源码自构建 |
| 演示可复现 | ✅ | `scripts/build-demo-fixture.sh` 在全新克隆上自动初始化演示仓库（幂等，四类合成数据） |
| AtomGit 露出 | ✅ | README 顶部 G-Star+Star+Fork+Download 徽章、托管与同步章节、AtomGit logo |
| 离线漏洞库管理 | ✅ | 设置页可导入 JSONL 索引并查看状态（条目数 / 更新时间 / 体积）；先验后写（解析失败或空库拒绝落盘），保持纯离线不做联网更新 |
| 设置页独立化 | ✅ | 检查器开关 / 阈值 / 历史范围 / 自定义规则 / 忽略规则编辑器拆出为独立设置页（视图 6，顶栏「设置」），连接页只留选仓库与规模预估；配置全局生效、存本机（唯一写入被检查仓库的是用户保存的 `.repobalance-ignore`） |
| 前端单元测试 | ✅ | Vitest（node 环境 + 内存 `localStorage`，零 DOM 依赖）：覆盖配置持久化 / 历史合并与迁移 / 评分口径 / 趋势聚合 / 批量调度状态机 / 忽略规则编辑器纯函数 / 更新流程状态机（44 用例），已接入 CI 质量门禁 |
| 应用内更新 | ✅ | `tauri-plugin-updater` + minisign 验签：设置页手动检查、展示说明与下载进度、安装后重启；**默认不联网**（无后台轮询），可选「启动时自动检查」；发布流水线按签名私钥产出 `.sig` 与 `latest.json`，密钥缺失时降级为「仅手动下载」并给出 warning |
| 分发清单与发布说明 | ✅ | `scripts/gen-packaging-manifests.sh` 按真实资产 SHA-256 生成 Homebrew Cask / Scoop / WinGet 清单并附到 Release（推送 tap / bucket / winget-pkgs 仍由维护者提 PR，见 [`distribution.md`](distribution.md)）；`scripts/release-notes.sh` 从 CHANGELOG 提取 Release 正文，缺小节时回退通用文案；Windows 额外产出便携 zip 供 Scoop 使用 |
| 双端同步 | ✅ | 同一提交推送 AtomGit + GitHub（G-Star 要求同步不超过 1 个月） |

### 关键缺口

1. PDF 导出未实现（报告内容含中文，需解决 CJK 字体嵌入，成本高于 JSON / Markdown / HTML）。
2. 漏洞库索引仍需手工生成：脚本已备且设置页已有 **GUI 导入入口**，但未随安装包分发，也不做联网自动更新（体检链路保持零网络）。
3. 包管理器清单只到「生成 + 附到 Release」：推送到 brew tap / Scoop bucket / winget-pkgs 需要各自仓库与凭证，由维护者按 [`distribution.md`](distribution.md) 提 PR；未做 macOS 公证与 Windows 代码签名。

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
- [x] **历史持久化迁移**
  - localStorage → 应用数据目录 `scan-history.json`（`std::fs` 直写，**不引入 fs 插件**，保持权限最小集）；
  - 原子写入（临时文件 + rename）；首次启动把旧 localStorage 数据一次性迁入后清理旧键；
  - 收益：突破 5MB 上限、不随清缓存丢失、可手工备份；为中期趋势分析铺路。
- [x] **大仓库性能**
  - 历史遍历增加 `--max-commits` / `--since-days` 限制（GUI「历史范围」分组，两者可叠加取交集）；
  - 扫描前展示规模预估（提交数/文件数/体积为精确值，历史对象数与耗时为采样外推）；
  - 受限扫描在 CLI、报告页、Markdown / HTML 导出中如实标注范围。
- [x] **历史导入导出入口**
  - 历史页头部新增「导入 / 导出」按钮；
  - 导入：文件对话框选 JSON → Rust 侧校验顶层为数组 → 前端按 id 合并去重（现有条目优先）→ 提示新增条数；
  - 导出：保存对话框（默认文件名 `repo-balance-history.json`）→ 写盘并提示文件名；
  - 走与报告导出相同的 `app.dialog()` Rust 侧 API，不新增 capabilities 权限。

### 短期验收标准

- [x] `cargo test --workspace` 全绿；每个新检查器覆盖「命中 / 不命中 / 边界阈值 / 忽略规则」四类用例（沿用现有中文测试名 + `TestRepo` 范式）；
- [x] 对真实中型仓库扫描，五个维度均产出真实结论（本仓库自体验证：五维全部计分，无占位假分）；
- [ ] G-Star 认证通过。

---

## 二、中期（2~6 个月）

**阶段目标：从「本地体检工具」演进为「团队仓库治理工具」。**

- [x] **依赖漏洞数据库**（Deps 维度升级）✅ 已于 2026-10-14 落地
  - 解析 Cargo.lock / Cargo.toml / package-lock.json / package.json / yarn.lock / go.mod / requirements.txt / pom.xml 的（包名, 版本）；
  - 集成 **OSV 离线库**（紧凑 JSONL 索引，由 `scripts/build-vuln-index.sh` 生成，**运行时零网络**）；
  - 差异化卖点成立：一个工具检测全栈依赖漏洞（单语言工具如 `cargo audit` 只能覆盖一种生态）；
  - 设计为**可选增强**：未放置索引时依赖维度退回结构检查，不报错、不扣分。
- [x] **配置驱动的自定义规则** ✅ 已于 2026-10-16 落地
  - 用户以 JSON 定义规则（正则 + 路径 glob + 严重度 + 维度 + 话术），GUI 表单管理与 JSON 导入导出（CLI `--rules`）；
  - 作为插件机制的轻量替代先行落地，验证真实需求后再投入插件系统；
  - 新增 `Checker::categories()` 默认方法：跨维度检查器如实声明覆盖维度，评分口径零妥协；
  - 格式与行为口径见 [`custom-rules.md`](custom-rules.md)。
- [x] **远程仓库体检** ✅ 已于 2026-10-19 落地
  - 输入 URL → 浅克隆到临时目录 → 本地扫描 → 自动清理（`TempDir` RAII，不落盘残留）；
  - CLI `rb scan-url <url> [--depth N]`；GUI 连接页新增远程 URL 直扫入口（`start_remote_scan` 命令）；
  - 报告内 `repo_path` 以 URL 为准（临时目录是实现细节）；本地路径源回退全量克隆（libgit2 本地传输不支持浅抓取）；
  - 复用 tempfile 依赖，保持纯本地语义。
- [x] **多仓库批量体检** ✅ 已于 2026-10-20 落地
  - 仓库队列串行扫描（多选目录 → 队列 → 逐个复用单仓扫描管线，无需新增 Rust 扫描命令）；
  - 汇总对比看板（`BatchView`）：整批进度 + 每仓状态 / 总分 / 问题数 / 严重数 / 用时，
    完成仓库按总分排序对比，平均分只统计已完成仓库；
  - 单仓失败只在队列里标红、不中断整批；取消批量会中止当前仓库并把剩余标记为跳过；
  - 完成的仓库照常写入扫描历史（趋势图可继续看演进），可一键跳回报告页查看详情；
  - 面向团队仓库治理场景，是认证后拉新与传播的主打功能。
- [x] **健康度趋势曲线** ✅ 已于 2026-10-20 落地
  - 基于已迁本地文件的历史数据，按仓库绘制「总分 + 五维」折线（时间轴为保存时间升序）；
  - 沿用五维既有语义色与 `filter.*` 文案；**N/A 维度断线不插值**（插值等于伪造未覆盖维度的结论）；
  - 图例点击隐藏 / 显示曲线，单点 tooltip 标注时间与取值；仓库下拉按最近扫描时间排序；
  - 数据源沿用历史保留策略（全局最多 20 条），页面如实标注该上限。
- [x] **设置页独立化** ✅ 已于 2026-10-20 落地
  - 检查器开关 / 阈值 / 历史范围 / 自定义规则 / 忽略规则编辑器从「连接页」拆出为独立设置页（视图 6，顶栏「设置」入口）；
  - 连接页只保留「选仓库（本地 / 远程 / 批量）→ 规模预估 → 开始体检」，并在底部给出设置页跳转；
  - 配置全局生效、存本机；唯一例外是忽略规则编辑器按用户显式操作写入仓库根目录的 `.repobalance-ignore`（见下条）。
- [x] **忽略规则编辑器** ✅ 已于 2026-10-23 落地
  - 设置页选定仓库后直接读写根目录 `.repobalance-ignore`：文件不存在时以空内容开始、保存即创建，写入为原子写（临时文件 + rename）；
  - 「试算」**不落盘**求值，口径与扫描同源（`parse_with_defaults` 与 `load_from` 同一实现，有测试锁定一致性）：内置默认 + 用户规则、最后匹配胜出；
  - 试算返回被忽略文件数 / 占比 / 字节总数（精确值，非采样外推）与最多 20 条路径样本，超出时如实标注「仅显示前 20 个」；
  - 静态检查「写了却不会生效」的三类规则：空模式（`/`、`!`）、反斜杠分隔、规则重复；**只提示不阻止保存**（注释与中间态是用户的自由）；
  - CLI `rb ignore <path>` 同口径只读校验，存在可疑规则时退出码 1，可放进 CI / pre-commit；
  - 语法与口径见 [`ignore-rules.md`](ignore-rules.md)；这是本应用唯一写入被检查仓库的操作，README 安全章节已如实标注。
- [x] **分发渠道扩展** ✅ 已于 2026-10-30 落地
  - 应用内更新（`tauri-plugin-updater`）：设置页手动检查 → 展示版本说明与下载进度 → 安装后重启；
    minisign 验签（公钥内置），发布流水线按私钥产出 `.sig` 与 `latest.json`；
  - **刻意不做后台静默轮询**：默认不联网是核心承诺，联网只在用户点击或显式开启
    「启动时自动检查」时发生；未配置签名私钥时流水线不失败，降级为「仅手动下载」；
  - Release notes 规范化：`scripts/release-notes.sh` 从 CHANGELOG 提取正文，
    缺小节时回退通用文案；`.github/release.yml` 作为按标签分类的兜底；
  - 分发清单：`scripts/gen-packaging-manifests.sh` 由真实资产 SHA-256 生成
    Homebrew Cask / Scoop / WinGet 清单，发布 job 自动附到 Release；Windows 额外
    产出便携 zip（Scoop 的 `bin` 直接指向 exe，不静默跑 NSIS）；
  - 推送 tap / bucket / winget-pkgs 需各自仓库与凭证，保持人工 PR；
    操作手册见 [`distribution.md`](distribution.md)。
- [x] **前端测试** ✅ 已于 2026-10-20 落地
  - 引入 Vitest（`npm run test`，node 环境 + 最小 `localStorage` 实现，不引入 jsdom）；
  - 覆盖 store 的配置持久化 / 历史合并与迁移 / 评分口径 / 趋势聚合 / 批量调度状态机（22 个用例），
    以及忽略规则编辑器的纯函数 `ui/src/utils/ignore.ts`（5 个用例，占比 / 字节格式化 / 脏标记 / 规则计数）；
  - 批量调度用「预置每轮结果 + 同步回推事件」的 IPC mock 在进程内跑完整批循环，含失败不中断、
    取消后剩余跳过、中止不留「进行中」三个关键回归；
  - 已接入 CI 质量门禁（`前端单元测试（Vitest）` 步骤），`README` 与 `package.json` 脚本同步。

### 中期验收标准

- v0.5.0 发布（批量体检、趋势图、远程体检、OSV 漏洞检测、前端单测、设置页独立化、应用内更新与包管理器清单均已落地），中期事项全部完成；
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
