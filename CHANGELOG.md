# 更新日志

本文件记录仓衡 RepoBalance 每个版本的显著变更。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

对应仓库 `master` 分支。**版本号未定**：检查器体系与评分口径已落地，但 PDF 导出等仍待补齐，待功能完整后再定版打 tag（进度见 [`docs/roadmap.md`](docs/roadmap.md)）。

### 安全

- 依赖升级修复 `cargo deny` 新公告：`tokio` → 1.54.1（RUSTSEC-2026-0386，`AsyncFd` I/O 安全问题，经由更新插件引入的 reqwest 传递依赖）、`libz-sys` → 1.1.31（RUSTSEC-2026-0418，构建脚本向 build 目录泄漏环境变量）
- `deny.toml` 许可证白名单新增 `CDLA-Permissive-2.0`（`webpki-root-certs` 的数据许可，内容为 Mozilla CA 根证书清单，非代码著佐权）

### 新增

- **应用内更新（可选、手动触发）**：设置页新增「软件更新」卡片，检查 → 展示版本说明与下载进度 → 安装 → 一键重启
  - `tauri-plugin-updater` + `@tauri-apps/plugin-updater` 负责检查、下载与安装，`tauri-plugin-process` 负责安装后重启；新增能力 `updater:default` / `process:default`
  - **默认不联网**：没有后台轮询；只有用户点击「检查更新」或显式开启「启动时自动检查」（本机偏好，默认关闭）时才发起请求，且只取版本元数据与安装包，不携带仓库内容、路径、扫描结果或设备标识
  - 安装包走 **minisign 验签**：公钥内置于 `tauri.conf.json`，签名缺失或不匹配一律拒绝安装
  - 发布侧新增 `crates/rb-app/tauri.updater.conf.json`（`createUpdaterArtifacts`），仅在 CI 配置了 `TAURI_SIGNING_PRIVATE_KEY` 时合并；未配置私钥不会让发布失败，只告警并降级为「手动下载」
  - 定位表述修正：README / SECURITY 的「纯本地零网络」改为「默认零网络 + 唯一的手动更新通道」，并新增「网络行为」章节（原文「不引入 HTTP 客户端」已不再成立，如实修正而非保留旧说法）
- **分发清单与发布说明自动化**
  - `scripts/gen-packaging-manifests.sh`：按真实发布资产的 SHA-256 生成 Homebrew Cask / Scoop / WinGet 三套清单；缺 dmg / setup.exe / 便携 zip 任一即失败，绝不生成指向不存在文件的清单
  - 发布工作流新增 `manifests` job：下载 Release 资产 → 计算校验和 → 生成清单 → 作为附件回传；Windows 额外产出便携 zip，Scoop 的 `bin` 直接指向 exe，不静默运行 NSIS 安装器
  - `scripts/release-notes.sh`：从 CHANGELOG 提取 Release 正文（容忍 `v` 前缀，缺小节时退出码 1）；`.github/release.yml` 作为按标签分类的兜底，Release 与 CHANGELOG 不再可能出现两套说法
  - 推送 brew tap / Scoop bucket / winget-pkgs 需要各自仓库与凭证，保持人工 PR，操作手册见 `docs/distribution.md`
- **更新流程单测**：14 个用例覆盖检查 / 发现新版本 / 重复检查释放旧句柄 / 下载进度（含无 Content-Length）/ 安装失败原地重试 / 连点去重 / 自动检查偏好持久化；连点去重用例发现并修复了「phase 在 await 之后才置位导致并发两次查询」的竞态
- **离线漏洞库管理（设置页）**：漏洞库此前只能手工拷贝到应用数据目录，现可在设置页导入 JSONL 索引并查看状态（条目数 / 更新时间 / 体积）
  - 新增 `import_vuln_index` / `vuln_index_info` 命令；**先验后写**：解析失败或空索引一律拒绝落盘，不会用损坏 / 空库覆盖既有可用索引
  - 保持**纯离线**定位：导入不做任何网络请求（索引仍由 `scripts/build-vuln-index.sh` 生成）
- **忽略规则编辑器（设置页）**：`.repobalance-ignore` 此前只能手工编辑，现可在设置页选定仓库后直接查看与修改
  - 新增 `read_ignore_rules` / `write_ignore_rules` / `check_ignore_rules` / `ignore_defaults` 四个 IPC 命令；保存走**原子写**（临时文件 + rename），失败不留残留临时文件
  - 「试算」**不落盘**求值，口径与扫描同源（`parse_with_defaults` 与扫描入口 `load_from` 共用同一实现，并有测试锁定两者一致）：内置默认 + 用户规则、最后匹配胜出
  - 试算返回被忽略文件数 / 占比 / 字节总数（遍历工作区文件元数据的**精确值**，非采样外推）与最多 20 条路径样本，超出时如实标注「仅显示前 20 个」
  - **静态检查**「写了却不会生效」的三类规则：空模式（`/`、`!` 等无有效路径段）、反斜杠分隔、规则重复；**只提示、不阻止保存**（带注释或待定条目的中间态是用户的自由）
  - CLI 新增 `rb ignore <path>`：同口径只读校验，存在可疑规则时退出码 1，可放进 CI / pre-commit
  - 这是本应用**唯一写入被检查仓库**的操作（只写根目录这一个文件，不触碰 git 对象库与提交历史），README 安全章节与设置页均已如实标注；语法与口径见 [`docs/ignore-rules.md`](docs/ignore-rules.md)
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
- **自定义规则：海量命中不再无限占用内存**：`find_offsets` 改为返回「偏移量（最多 4096）+ 真实命中总数」——`x*` 这类几乎逐字节命中的正则会让偏移数组随文件体积线性膨胀（1 MiB 文件可达百万级 `usize`）；结论里的「命中 N 处」仍是真实总数，不受行号截断影响（实测 37k 命中的结论正常产出，内存有界）
- **自定义规则：同一行多处命中只列一次行号**（原先会出现「行 1, 1, 1」这类噪音）
- **批量体检：历史写盘改为串行 await**：整批会连续写入多次扫描历史，原先多个 `saveScanHistory` 可能并发落盘并乱序完成，历史文件会留下旧快照（历史回退）；现每轮 `await` 后再扫下一个仓库
- **远程仓库体检：HTTPS 克隆一律失败（`there is no TLS stream available`）**：`git2` 声明为 `default-features = false`，而 git2 0.21 的 `default` 本身为空，编译出的 libgit2 **不带任何 TLS 后端**——所有 HTTPS 远程 URL 都在克隆阶段失败，扫描中止后回到接入页（表现即「远程体检不生效、进不到报告页」）
  - 根因修复：显式开启 `https` feature。各平台后端 macOS = SecureTransport、Windows = WinHTTP、Linux = 系统 OpenSSL（CI 已装 `libssl-dev`），不引入 vendored OpenSSL
  - **远程体检失败时错误不可见**：接入页错误卡片原先只在「已选仓库」时渲染，而远程来源没有本地路径（`selectedPath` 为空），用户只看到「回到接入页」却不知道原因；现错误卡片独立于 `selectedPath` 渲染，并给出「返回接入页」出口
  - `ssh://`（以及未配置 `insteadOf` 的 `git@host:`）仍不支持——未启用 git2 的 `ssh` feature；但不再抛出无信息量的 `unsupported URL protocol`，改为「改用 HTTPS 地址 / 用 `url.<HTTPS 前缀>.insteadOf` 改写」的可执行提示
  - 进度页标题在远程体检时显示目标 URL（原先因 `selectedPath` 为空而空白）
  - 新增 store 回归用例：远程体检「记住 URL + 清空本地目标 / 完成后进报告页 / 失败留在接入页且错误可见」

### 变更

- **设置页独立化**：检查器开关 / 阈值 / 历史范围 / 自定义规则 / 忽略规则编辑器从连接页拆出为独立设置页（视图 6，顶栏「设置」入口）
  - 连接页只保留「选仓库（本地 / 远程 / 批量）→ 规模预估 → 开始体检」，并在底部给出设置页跳转
  - 配置全局生效、保存在本机；唯一写入被检查仓库的是用户在忽略规则编辑器里保存的 `.repobalance-ignore`；改动在下一次扫描时生效
- 报告与导出中的维度评分改为可空；未覆盖维度在界面显示 N/A
- `.repobalance-ignore` 语义扩展：内置默认忽略项与用户规则合并，用户否定规则优先级更高

### 工程与流程

- **前端单元测试（Vitest）**：`npm run test` 覆盖 store 的配置持久化 / 历史合并与迁移 / 评分口径 / 趋势聚合 / 批量调度状态机，以及忽略规则编辑器的纯函数 `ui/src/utils/ignore.ts`（30 用例）
  - node 环境 + `src/test/setup.ts` 提供的最小 `localStorage` 实现，不引入 jsdom，依赖与启动开销最小
  - 批量调度用「预置每轮结果 + 同步回推事件」的 IPC mock 在进程内跑完整批循环，覆盖失败不中断、取消后剩余跳过、`resetToConnect` 中止不留「进行中」三个关键回归
  - 为便于单测，`parseHistory` / `mergeHistory` / `loadConfig` / `totalScoreOf` / `totalOrNullOf` / `repoNameOf` 以具名导出暴露（应用代码仍只消费 store 实例）
  - CI 质量门禁新增「前端单元测试」步骤
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
