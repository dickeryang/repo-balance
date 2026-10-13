# 更新日志

本文件记录仓衡 RepoBalance 每个版本的显著变更。
格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [未发布]

对应仓库 `master` 分支。**版本号未定**：五个检查器与评分口径已落地，但历史持久化迁移、PDF 导出等仍待补齐，待功能完整后再定版打 tag（进度见 [`docs/roadmap.md`](docs/roadmap.md)）。

### 新增

- **四个新检查器**，五个评分维度首次全部由真实规则产出结论：
  - `secrets`（安全）：私钥头 / AWS Access Key / GitHub·GitLab token / `.env` 赋值 / 高熵密钥赋值，正则 + 香农熵双通道；evidence 构造时即掩码，示例值走停用词白名单
  - `zombie-branches`（分支）：已合并未删除、超 N 天未活跃（默认 90 天，可配）、无远端追踪
  - `commit-habits`（历史）：超大提交（>50 文件 Warning / >200 Critical）、低质提交信息占比、深夜提交比例（按提交时区换算）
  - `deps-health`（依赖）：锁定文件缺失 / 未提交 / 被删、同生态多包管理器混用
- **切片体系扩展**：`SliceData` 由两个变体扩展为 `FileContents` / `FullHistory` / `BranchGraph` / `DependencyFiles`；`FileContents` 载荷升级为携带文本内容（单文件 1 MiB、全仓 32 MiB 预算、二进制探测跳过）
- **进度事件增强**：`ScanProgress` 增加 `checkerId` 与 `findingCount`，扫描页检查器列表由进度事件实时驱动（待运行 → 运行中 → 完成 + 结论数）
- **配置面板**：五个检查器独立开关 + 僵尸分支未活跃天数；新增 `zombieBranchDays` 配置项
- **忽略规则内置默认项**：`target/`、`node_modules/` 等构建产物目录默认忽略（可用 `!` 否定），依赖切片同步过滤忽略目录
- **英文版文档**：`README_EN.md`（双语互链）、`SECURITY.md`、`CHANGELOG.md`、`CODE_OF_CONDUCT.md`、`CONTRIBUTING.md`、issue/PR 模板、`.editorconfig` / `.gitattributes`
- **演示能力**：`scripts/build-demo-fixture.sh` 支持在全新克隆上自动初始化演示仓库（幂等，产出四类合成问题数据）

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
