# 更新日志

本文件记录仓衡 RepoBalance 每个版本的显著变更。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

对应仓库 `master` 分支。**版本号未定**：检查器体系与评分口径已落地，但 PDF 导出等仍待补齐，待功能完整后再定版打 tag（进度见 [`docs/roadmap.md`](docs/roadmap.md)）。

### 新增

- **多仓库批量体检**：连接页「批量选择目录」→ 串行队列 → 汇总对比看板（新视图 `BatchView`）
  - 队列逐仓复用单仓扫描管线（`scan-done` / `scan-error` 按代际号收尾，无需新增扫描命令）；**单仓失败只在队列里标红，不中断整批**；取消批量会中止当前仓库并把剩余标记为跳过
  - 看板：整批进度 + 每仓状态 / 总分 / 问题数 / 严重数 / 用时；已完成仓库按总分排序对比，平均分只统计已完成仓库（未覆盖维度不参与），失败原因直接展示在对应行
  - 完成的仓库照常写入扫描历史（趋势图可继续看演进），并可一键跳回报告页查看详情
  - 新增 `select_directories` 命令（多选目录，**不预校验**每个目录是否为 git 仓库——批量场景下「一个坏目录中断整批」不可接受）
- **健康度趋势曲线**：历史页新增「健康度趋势」卡片，按仓库绘制「总分 + 五维」折线（`TrendChart`，纯内联 SVG，无图表依赖）
  - N/A 维度**断线不插值**，不把「无结论」画成断崖下跌；图例可点击隐藏 / 显示单条曲线，单点 tooltip 标注时间与取值
  - 仓库下拉按最近扫描时间排序并显示扫描次数；X 轴刻度按点数抽稀（最多约 6 个，保留首尾）
  - 数据源为本地扫描历史，页面如实标注「全局最多保留 20 条」的上限；不足两次扫描时提示补扫
- **自定义规则（配置驱动检查器 `custom-rules`）**：用户以 JSON 定义「正则 + 路径 glob + 严重度 + 维度 + 话术」，把团队自有规约变成可复现的体检结论
  - 正则由 `regex` 执行（线性时间、无回溯），用户正则不会拖垮扫描；命中按「规则 × 文件」聚合，证据含命中次数、前 5 处行号与**已掩码**的首个匹配样本
  - 单条规则最多产出 50 条文件级结论，超出时追加一条 `info` 结论如实说明「共命中 N 个文件，仅列出前 50 个」，避免宽泛正则淹没报告
  - GUI：连接页「检查器配置」新增「自定义规则」分组（表单即时校验 ID 唯一 / 正则可编译 / 必填项），支持 JSON 导入导出；CLI 新增 `--rules FILE`
  - 规则非法（JSON 结构 / 正则 / ID 重复）一律**快速失败**，不做静默降级；无规则时不注册该检查器，避免无规则的维度假满分
  - 新增契约 `Checker::categories()`（默认返回 `category()`）：跨维度检查器如实声明覆盖维度，评分口径不变
  - 格式与行为口径见 [`docs/custom-rules.md`](docs/custom-rules.md)
- **四个新检查器**，五个评分维度首次全部由真实规则产出结论：
  - `secrets`（安全）：私钥头 / AWS Access Key / GitHub·GitLab token / `.env` 赋值 / 高熵密钥赋值，正则 + 香农熵双通道；evidence 构造时即掩码，示例值走停用词白名单
  - `zombie-branches`（分支）：已合并未删除、超 N 天未活跃（默认 90 天，可配）、无远端追踪
  - `commit-habits`（历史）：超大提交（>50 文件 Warning / >200 Critical）、低质提交信息占比、深夜提交比例（按提交时区换算）
  - `deps-health`（依赖）：锁定文件缺失 / 未提交 / 被删、同生态多包管理器混用
- **切片体系扩展**：`SliceData` 由两个变体扩展为 `FileContents` / `FullHistory` / `BranchGraph` / `DependencyFiles`；`FileContents` 载荷升级为携带文本内容（单文件 1 MiB、全仓 32 MiB 预算、二进制探测跳过）
- **进度事件增强**：`ScanProgress` 增加 `checkerId` 与 `findingCount`，扫描页检查器列表由进度事件实时驱动（待运行 → 运行中 → 完成 + 结论数）
- **配置面板**：五个检查器独立开关 + 僵尸分支未活跃天数；新增 `zombieBranchDays` 配置项
- **忽略规则内置默认项**：`target/`、`node_modules/` 等构建产物目录默认忽略（可用 `!` 否定），依赖切片同步过滤忽略目录
- **英文版文档**：`README_EN.md`、`CONTRIBUTING_EN.md`、`SECURITY_EN.md`、`CODE_OF_CONDUCT_EN.md`，与中文版双向互链
- **治理文件**：`SUPPORT.md`（含未签名安装 / 误报处理 / 评分口径等 FAQ）、`GOVERNANCE.md`（维护模型、角色演进、决策流程与项目底线）、`AUTHORS.md`、`CITATION.cff`（通过 CFF 1.2.0 官方 schema 字段校验）；`CONTRIBUTING.md`、`CHANGELOG.md`、issue/PR 模板、`.editorconfig` / `.gitattributes`
- **演示能力**：`scripts/build-demo-fixture.sh` 支持在全新克隆上自动初始化演示仓库（幂等，产出五类合成问题数据，新增未解决冲突标记样例）
- **合并冲突残留检查器**（Structure 维度）：检出工作区与历史提交中残留的 `<<<<<<<` / `=======` / `>>>>>>>` 冲突标记
  - 三重防误报：完整三元组按行序齐备才计数（单独 `=======` 永不命中）、文档示例停用词白名单、复用 `.repobalance-ignore`
  - 历史口径仅说明「通常无需处理」并给出可选的 `git filter-repo` 命令，不强制重写历史
  - 新增 `HistoryFileContents` 切片与历史文件内容采集（受预算约束）
- **检查器耗时统计**：`ScanReport` 新增 `checkerTimings`（各检查器 `check` 阶段毫秒耗时，按注册顺序）
- **大仓库性能闸门**：历史遍历支持「最多 N 个提交」与「最近 N 天」两个限制条件
  - CLI 新增 `--max-commits N` / `--since-days N`（两者可叠加取交集）
  - GUI 配置面板新增「历史范围」分组（留空 = 不限）
  - 受限扫描在 CLI 输出、报告页、Markdown / HTML 导出中**如实标注**范围，避免受限结论被误读为全量结论
- **扫描前规模预估**：新增 `scan_estimate` 命令，返回提交数 / 文件数 / 工作区体积（精确值）与历史对象数、预计耗时（采样外推的粗估）
  - 连接页新增「扫描规模预估」卡片：分 small / medium / large / huge 四档，大仓库给出建议限制并提供「应用建议」一键收敛
- **扫描历史迁移到应用数据目录**：由 localStorage 改为 `<app_data_dir>/scan-history.json`（原子写入）
  - 突破 localStorage 约 5MB 上限，不随浏览器缓存清理丢失，可直接备份
  - 首次启动自动把旧版 localStorage 数据一次性迁入文件后清理旧键
- **依赖漏洞检测（离线漏洞库）**：接入 OSV 风格离线索引，依赖维度由「仅结构检查」升级为「结构 + 已知漏洞」
  - 新增切片 `DependencyFileContents`，解析 Cargo.lock / Cargo.toml / package-lock.json / package.json / yarn.lock / go.mod / requirements.txt / pom.xml 的（包名, 版本）
  - 自写跨生态版本比较（semver 与 Maven / PEP 440 常见子集），无新依赖
  - 结论 id 为 `deps-health.vuln:<公告号>:<包名>`；按「公告 + 包名」去重，版本优先采信锁定文件
  - **可选增强**：未接入漏洞库时依赖维度行为与接入前完全一致（不报错、不扣分）
  - CLI 新增 `--vuln-db FILE`；桌面端从应用数据目录 `vuln-index.jsonl` 自动载入
  - 新增 `scripts/build-vuln-index.sh`：从 OSV 下载并转为紧凑 JSONL（**开发者手工执行，运行时仍零网络**）
- **历史导入导出入口**：历史页新增「导入 / 导出」按钮
  - 导入：文件对话框选 JSON，Rust 侧校验顶层为数组（非法/非数组直接报错），前端按 id 合并去重（现有条目优先）并提示新增条数
  - 导出：保存对话框（默认文件名 `repo-balance-history.json`），写盘后提示文件名
  - 复用既有的 `app.dialog()` Rust 侧 API，不新增 capabilities 权限，权限最小集保持不变

### 修复

- **安全：`git2` 0.19 → 0.21**（依赖树审计发现 3 条 unsound 公告）
  - RUSTSEC-2026-0008（`Buf` 解引用产生未定义行为，修复版 ≥ 0.20.4）
  - RUSTSEC-2026-0183（`Remote::list()` 空指针，修复版 ≥ 0.21.0）
  - RUSTSEC-2026-0184（`BlameHunk` 空指针签名，修复版 ≥ 0.21.0）
  - 适配 0.21 的 API 变更：`Commit::summary()` 由 `Option<&str>` 变为 `Result<Option<&str>>`（新增容错取值 helper），`TreeEntry::name()` 由 `Option<&str>` 变为 `Result<&str>`（非 UTF-8 名跳过，语义不变）
  - 剩余 `glib` 0.18 不健全公告（RUSTSEC-2024-0429）登记为带说明的豁免：来自 Linux GTK 传递依赖，修复需上游升级 gtk-rs 栈
- **评分口径**：无检查器覆盖的维度此前恒为 100 分（雷达图 4/5 为占位假数据），现改为 N/A；总分 = 已覆盖维度平均
- **打包后前端白屏**：`cargo build --release -p rb-app` 未启用 `custom-protocol`，`tauri-build` 置 `cfg(dev)` 导致不内嵌前端资源、改从 `devUrl` 加载。补齐 `custom-protocol` feature 声明并修正 README 构建命令
- **窗口初始位置**：窗口可能被放至非主显示器，现固定居中于主屏
- **前端 ESLint 配置失效**：`@typescript-eslint/recommended` 引用错误导致 `npm run lint` 直接崩溃，现修正解析器分区与 `dist` 排除
- **依赖误报**：`node_modules/` 内的嵌套清单/锁定文件不再被当作仓库自身依赖口径
- **格式归一**：`rustfmt` 对 21 个文件做纯格式化（此前代码不符合自己声明的格式规范）
- **MSRV 声明**：README/`Cargo.toml` 由「Rust 1.77+」修正为 **1.88+**（依赖树实测下界，1.77 无法构建）
- **限流假阳性**：限制历史范围后，窗口最旧提交会「继承」父树中本就已存在的 blob，被误判为超大提交（实测出现「docs 提交引入 154 个文件」）。现对窗口边界提交做父树比对，只保留窗口内真正引入的 blob

### 变更

- 报告与导出中的维度评分改为可空；未覆盖维度在界面显示 N/A
- `.repobalance-ignore` 语义扩展：内置默认忽略项与用户规则合并，用户否定规则优先级更高

### 工程与流程

- **CI 质量门禁**（新增 `.github/workflows/ci.yml`）：rustfmt / clippy（`-D warnings`）/ 单元测试 / 前端 lint 与构建，与 PR 模板承诺的门禁一一对应
- **供应链审计**：`deny.toml` + cargo-deny（许可证白名单、漏洞、依赖来源），CI 独立 job 执行；Dependabot 每周自动升级（Rust / npm / Actions）
- **覆盖率**：rb-core 覆盖率摘要（cargo-llvm-cov）纳入 CI
- **评审归属**：新增 `.github/CODEOWNERS`
- **打包矩阵**：新增 Linux（`.deb` / `.AppImage`），消除 README 声称产出 Linux 包但 CI 不构建的不一致
- **AtomGit 流水线**：增加「质量门禁」阶段（格式 / clippy / rb-core 测试）

## [0.1.0] - 2026-10-06

首个发布版本：可用的本地 Git 仓库体检台骨架。

### 新增

- Tauri 2 桌面壳（Rust 原生 WebView）+ Vue 3 / TypeScript / Pinia / Vite 4 前端
- 检查器插件体系：`Checker` trait、注册表、两阶段编排（plan → merge → collect → check）、`ScanContext` 切片上下文
- 大文件检查器：工作区 + 历史双通道，阈值可配（默认 1 MiB）
- 扫描编排：阶段进度事件（snapshot / workdir / history / check / done）、取消并产出部分报告
- 报告导出：JSON / Markdown / HTML（自包含，内联 SVG 雷达图）
- 扫描历史：localStorage 持久化 + 两次扫描差异对比（新增 / 消除的问题项）
- 忽略规则：`.repobalance-ignore`（gitignore 风格，含否定规则）
- 检查器配置：阈值调整与启用/禁用
- 浅色 / 深色主题，雷达图全屏
- 国际化：中英文界面切换（自写字典，localStorage 持久化）
- CLI 模式：`rb scan <path> [--ci|--json|--quiet]`
- CI 集成：GitHub Actions 自身体检 + 跨平台打包 workflow，AtomGit 流水线示例

### 安全

- CSP `default-src 'self'`、权限最小集（无 `fs` / `shell`）、纯本地零网络、evidence 掩码

[未发布]: https://github.com/dickeryang/repo-balance/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dickeryang/repo-balance/releases/tag/v0.1.0
