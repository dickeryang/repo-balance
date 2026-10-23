# 仓衡 RepoBalance

[English](README_EN.md) | 简体中文

[![AtomGit G-Star](https://atomgit.com/dickeryang/repo-balance/star/new_badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Star](https://atomgit.com/dickeryang/repo-balance/star/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Fork](https://atomgit.com/dickeryang/repo-balance/fork/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Download](https://atomgit.com/dickeryang/repo-balance/download/badge.svg)](https://atomgit.com/dickeryang/repo-balance/releases)
[![质量门禁](https://github.com/dickeryang/repo-balance/actions/workflows/ci.yml/badge.svg)](https://github.com/dickeryang/repo-balance/actions/workflows/ci.yml)

用华为云码道 CodeArts 代码智能体独立开发一个跨平台桌面应用「仓衡 · Git 仓库健康度体检台」，对本地 Git 仓库做结构化健康体检：大文件、僵尸分支、提交习惯、依赖健康、敏感信息残留扫描，输出可导出的体检报告。

## 托管与同步

本项目国内官方托管在 **AtomGit**，并参与 [AtomGit G-Star 开源摘星计划](https://atomgit.com/GitCode/G-Star_guiding)。

<a href="https://atomgit.com/dickeryang/repo-balance"><img src="docs/AtomGit-logo.svg" alt="AtomGit" width="28"></a> [国内 AtomGit 托管：https://atomgit.com/dickeryang/repo-balance](https://atomgit.com/dickeryang/repo-balance)

| 平台 | 地址 | 说明 |
|---|---|---|
| AtomGit（国内官方） | https://atomgit.com/dickeryang/repo-balance | 国内官方托管，G-Star 认证所在平台 |
| GitHub（国际镜像） | https://github.com/dickeryang/repo-balance | 与 AtomGit 同步更新 |

> **同步约定**：每次发布同一提交双端推送，保持两端内容一致。G-Star 要求多平台维护时同步更新不得晚于其他平台超过 1 个月，否则 AtomGit 有权取消认证；本项目以「双端同步推送」为发布标准动作。

## 演示

**选择仓库并启动体检：**

![选择仓库演示](docs/repobalance-git-demo-repo-select.gif)

**扫描历史对比（新增/消除的问题项）：**

![历史对比演示](docs/repobalance-git-demo-history-compare.gif)

### 生成演示仓库（体验全部检查项）

`demo-repo/` 是本地生成的合成样例（已被 `.gitignore` 忽略，克隆后并不存在）。一条命令即可创建：

```bash
bash scripts/build-demo-fixture.sh
```

脚本**幂等**、不重写或 amend 既有提交，产出四类合成数据：

| 合成内容 | 演示的检查项 |
|---|---|
| `big.bin`（5 MiB，已提交且留在工作区） | 大文件 · Warning |
| `.env`（含赋值配置） | 敏感信息残留 · Warning |
| `old-branch`（含未合并 `wip` 提交） | 僵尸分支 · Info |
| `leaked.txt`（`DEMO KEY` 占位块） | 敏感信息规则的「停用词白名单」形态（不产出结论） |

随后可在应用内点击「试试演示仓库 demo-repo」，或用 CLI 直接体检：

```bash
cargo run -p rb-core --bin rb scan ./demo-repo
```

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面壳 | Tauri 2（Rust 原生 WebView，零 Electron） |
| 前端 | Vue 3 + TypeScript + Pinia + Vite 4 |
| 检查器内核 | Rust（git2 + rayon + serde + thiserror） |
| 通信 | Tauri IPC（invoke 命令 + emit 事件） |

## 前置环境

| 工具 | 最低版本 | 说明 |
|---|---|---|
| Rust | 1.88+ | 依赖树 MSRV 下界（`darling` / `plist` / `time` 声明 1.88）；`rustup update stable` |
| Node.js | 14.18+ | Vite 4 最低要求 |
| npm | 6+ | 或 pnpm / yarn |
| Windows SDK | 10+ | 仅 Windows 构建需要（dbghelp.lib + advapi32.lib） |

### 安装 Tauri CLI

```bash
cargo install tauri-cli --version "^2"
```

## 开发模式

### 1. 安装前端依赖

```bash
cd ui
npm install --registry https://registry.npmjs.org/
cd ..
```

> 若 npm 默认源为内网镜像，需加 `--registry https://registry.npmjs.org/` 指向官方源（`@tauri-apps/api` 等包需从官方源拉取）。

### 2. 启动开发服务器

**方式一（推荐）：Tauri dev 一体化启动**

```bash
cargo tauri dev
```

此命令自动启动 Vite devServer（5173 端口）+ Rust 热重载，打开 1200×800 桌面窗口。

**方式二：分终端启动**

```bash
# 终端 1：前端 devServer
cd ui && npm run dev

# 终端 2：Rust 侧
cargo run -p rb-app
```

### 3. 质量门禁

以下命令与 CI（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)）完全一致，提交前请本地跑通：

```bash
# Rust 格式（rustfmt）
cargo fmt --all -- --check

# Rust 静态检查（零 warning 门禁）
cargo clippy --all-targets --workspace -- -D warnings

# Rust 测试
cargo test --workspace

# 前端 lint + 单测 + 构建
cd ui && npm run lint && npm run test && npm run build
```

> CI 还包含两个独立 job：**依赖许可证与漏洞审计**（cargo-deny，门禁配置见 [`deny.toml`](deny.toml)）与
> **rb-core 覆盖率摘要**（cargo-llvm-cov）。

## 打包发布

### 生产构建

```bash
cargo tauri build
```

此命令依次执行：
1. `npm run build`（前端产出 `ui/dist/`）
2. `cargo build --release -p rb-app --features custom-protocol`（Rust 编译优化，内嵌前端资源）
3. 打包为平台安装包

产物位于 `target/release/bundle/`：

| 平台 | 产物 | 路径 |
|---|---|---|
| Windows | `.msi` 安装包 + `.exe` | `target/release/bundle/msi/` |
| Windows | NSIS `.exe` 安装包 | `target/release/bundle/nsis/` |
| macOS | `.dmg` + `.app` | `target/release/bundle/dmg/` |
| Linux | `.deb` + `.AppImage` | `target/release/bundle/deb/` |

### 安装与首次运行（未签名说明）

发布包**未做代码签名**（开源项目的常见状态），首次运行会被系统拦截：

| 平台 | 系统提示 | 处理方式 |
|---|---|---|
| macOS | 「Apple 无法检查其是否包含恶意软件」 | 右键应用 → 打开；或 `xattr -dr com.apple.quarantine "/Applications/仓衡.app"` |
| Windows | SmartScreen 蓝色警告 | 点「更多信息」→「仍要运行」 |

介意风险可从源码自行构建（见上方构建命令），全部构建步骤均可本地复现。

### 仅构建前端（不打包桌面应用）

```bash
cd ui && npm run build
```

产出 `ui/dist/`（index.html + assets/），可用于 Web 预览或嵌入其他容器。

### 仅构建 Rust 二进制（不打包安装包）

```bash
cargo build --release -p rb-app --features custom-protocol
```

产出 `target/release/rb-app`（Windows 为 `rb-app.exe`），前端资源已内嵌，可直接运行。

> ⚠️ 必须带 `--features custom-protocol`。缺省时 tauri 编译为 dev 模式
> （`tauri-build` 置 `cfg(dev)`），不内嵌前端资源而改从 `devUrl`
> （`http://localhost:5173`）加载，无 dev server 时界面全白。

## CLI 模式（无需 GUI）

rb-core 可独立编译为命令行工具，不依赖 Tauri：

```bash
cargo run -p rb-core --bin rb scan ./demo-repo
```

输出仓库摘要 + 体检结论（finding 列表含严重度、证据、修复建议）。

远程仓库同理（浅克隆到临时目录后复用同一条扫描管线，退出即清理，不在本机落盘）：

```bash
cargo run -p rb-core --bin rb scan-url https://host/repo.git
```

除体检外，还可**只读校验忽略规则**（与 GUI 编辑器「试算」同一口径，不写任何文件）：

```bash
cargo run -p rb-core --bin rb ignore ./demo-repo
```

打印用户规则条数 + 内置默认条数，并列出「写了却不会生效」的规则（空模式 / 反斜杠 / 重复）；存在可疑规则时退出码为 1，可放进 CI 或 pre-commit。

## 项目结构

```
repo-balance/
├── crates/
│   ├── rb-core/              # 检查器内核（零 tauri 依赖）
│   │   ├── src/
│   │   │   ├── checker/      # Checker trait + 注册表 + 切片计划 + 上下文 + 六个内置检查器
│   │   │   ├── engine/       # 扫描编排（plan → merge → collect → check）
│   │   │   ├── git/          # git2 只读门面（类型不逃逸）
│   │   │   ├── model/        # 数据模型（RepoSnapshot/Finding/ScanReport）
│   │   │   ├── rules.rs      # 自定义规则模型（校验 + 编译为线性时间正则）
│   │   │   └── bin/rb.rs     # CLI 入口
│   │   └── build.rs          # Windows advapi32 链接补齐
│   └── rb-app/              # Tauri 2 桌面壳
│       ├── src/
│       │   ├── main.rs       # Tauri Builder 入口
│       │   ├── commands.rs   # IPC 命令（扫描 / 历史 / 规则 / 忽略规则 / 导出）
│       │   ├── dto.rs        # camelCase DTO 镜像层
│       │   ├── error.rs      # 错误映射
│       │   └── state.rs      # 扫描状态管理
│       ├── capabilities/     # 权限最小集
│       ├── icons/            # 应用图标
│       ├── tauri.conf.json   # Tauri 配置（CSP/窗口/打包）
│       └── build.rs          # tauri-build
├── ui/                       # Vue 3 前端工程
│   ├── src/
│   │   ├── views/            # 六视图（接入/扫描/报告/历史/批量看板/设置）
│   │   ├── components/       # 通用组件（TopBar/RadarChart/TrendChart）
│   │   ├── stores/           # Pinia 状态管理
│   │   ├── ipc/              # IPC 封装（命令+事件）
│   │   ├── utils/            # 纯函数（与渲染分离，便于单测）
│   │   ├── styles/           # 主题样式（14 CSS 变量）
│   │   ├── test/             # 测试环境准备（内存 localStorage）
│   │   └── types/            # TS 类型镜像
│   ├── package.json
│   ├── vite.config.ts
│   └── vitest.config.ts      # Vitest 配置（node 环境，store 与纯函数单测）
├── docs/                     # 文档（路线图 / 自定义规则 / 忽略规则 / CI 集成 / 演示 GIF）
│   ├── roadmap.md            # 权威路线图（现状盘点 / 阶段目标 / 验收标准）
│   ├── custom-rules.md       # 自定义规则 JSON 格式与行为口径
│   ├── ignore-rules.md       # 忽略规则语法、内置默认项与编辑器 / CLI 口径
│   └── ci-integration.md     # CI 接入指南
├── design/                   # 品牌资产
│   ├── icon.svg              # 应用图标源文件
│   ├── preview.png           # 1024×1024 图标预览（可直接用作代码平台项目头像）
│   └── preview-64.png        # 64×64 图标
├── scripts/                  # 夹具脚本（build-demo-fixture.sh）
└── Cargo.toml                # workspace 根
```

## 安全设计

- **CSP**: `default-src 'self'`（禁止外部资源加载）
- **权限最小集**: `core:default` + `dialog:allow-open` + `core:event:default`（无 fs/shell）
- **路径来源**: 仅系统目录选择对话框，无自由文本输入
- **本地落盘范围**: 除用户主动导出的报告、应用数据目录的 `scan-history.json`（扫描历史）外，仅有一处写入被检查仓库——用户在设置页保存的 `.repobalance-ignore`（忽略规则必须随仓库走，与 `.gitignore` 同理）；不触碰 git 对象库与提交历史
- **纯本地零网络**: 不发送任何数据到远程服务器
- **evidence 掩码**: 证据在 rb-core 构造时掩码，导出/IPC 不复原
- **rb-core 零 tauri 依赖**: 内核可独立编译为 CLI，不耦合 UI 框架

## 规划

> 完整路线图（含现状盘点、阶段目标与验收标准）见 [`docs/roadmap.md`](docs/roadmap.md)。
> 以下仅为概要；未完成的功能一律标记为未完成，文档不超前于实现。

### 当前版本已完成（v0.1.x）

- [x] **六个检查器全部落地**：大文件（结构）、敏感信息残留（安全）、僵尸分支（分支）、提交习惯（历史）、依赖健康（依赖）、合并冲突残留（结构），五维均有真实结论
- [x] **合并冲突残留检查器**：检出工作区与历史提交中残留的 `<<<<<<<` / `=======` / `>>>>>>>` 冲突标记；三重防误报（完整三元组才计数、文档示例停用词白名单、复用忽略规则），历史口径给出「通常无需处理」的说明而非强制改史
- [x] **敏感信息检查器**：私钥头 / AWS Access Key / GitHub·GitLab token / `.env` 赋值 / 高熵密钥赋值双通道（正则 + 香农熵），evidence 构造时即掩码，内置示例值停用词白名单
- [x] **僵尸分支检查器**：已合并未删除、超 N 天未活跃（默认 90 天，可配）、无远端追踪
- [x] **提交习惯检查器**：超大提交（>50 文件 Warning / >200 Critical）、低质提交信息占比、深夜提交比例（按提交时区换算）
- [x] **依赖健康检查器**：锁定文件缺失 / 未提交 / 被删、同生态多包管理器混用；接入离线漏洞库后追加已知漏洞结论（解析 8 类依赖清单，跨生态版本匹配，未接入索引时退回结构检查）
- [x] **评分口径修复**：无检查器覆盖的维度显示 N/A 而非 100，总分 = 已覆盖维度平均
- [x] **报告导出**：JSON / Markdown / HTML（自包含，内联 SVG 雷达图）
- [x] **扫描历史**：应用数据目录 `scan-history.json` 持久化（旧 localStorage 数据自动迁移），支持两次扫描结果对比（新增/消除的问题项），并可导入 / 导出 JSON 备份
- [x] **忽略规则**：`.repobalance-ignore`，含否定规则（`!pattern`）；内置默认忽略构建产物目录（`target/`、`node_modules/` 等，可 `!` 否定）
- [x] **忽略规则编辑器（设置页）**：选定仓库后直接查看 / 修改仓库根目录的 `.repobalance-ignore`；「试算」不落盘求值（内置默认 + 用户规则，口径与扫描完全一致），给出被忽略文件数 / 占比 / 体积与路径样本，并静态检查「写了却不会生效」的规则（空模式 / 反斜杠 / 重复），保存走原子写；CLI `rb ignore <path>` 同口径只读校验（语法与口径见 [`docs/ignore-rules.md`](docs/ignore-rules.md)）
- [x] **CI 集成**：GitHub Actions 流水线，CLI 模式跑体检并上传报告
- [x] **国际化**：界面中英文切换（i18n）
- [x] **大仓库性能闸门**：历史遍历支持「最多 N 个提交」/「最近 N 天」（CLI `--max-commits` / `--since-days`，GUI「历史范围」），受限扫描在 CLI、报告页与导出中如实标注
- [x] **扫描规模预估**：连接页展示提交数 / 文件数 / 体积与预计耗时（对象数与耗时为采样外推的粗估），大仓库给出建议限制并支持一键应用
- [x] **自定义规则**：以 JSON 定义「正则 + 路径 glob + 严重度 + 维度 + 话术」，把团队自有规约变成可复现结论；GUI 表单管理 + JSON 导入导出，CLI `--rules FILE`；正则线性时间匹配，命中样本掩码，单规则 50 文件上限并如实提示截断（格式见 [`docs/custom-rules.md`](docs/custom-rules.md)）
- [x] **远程仓库体检**：输入 URL 浅克隆到临时目录（`TempDir` RAII 自动清理）后复用本地扫描管线，报告路径以 URL 为准；CLI `rb scan-url <url> [--depth N]`，GUI 连接页支持远程 URL 直扫。HTTPS 地址开箱可用；`git@host:` 依赖 git 配置的 `insteadOf` 改写，`ssh://` 未启用（会给出改用 HTTPS 的提示）
- [x] **健康度趋势分析**：历史页按仓库绘制「总分 + 五维」折线（内联 SVG，无图表依赖）；N/A 维度断线不插值、图例可隐藏曲线、单点 tooltip 标注时间与取值
- [x] **多仓库批量体检**：一次多选目录形成串行队列，单仓失败不中断整批；汇总对比看板展示整批进度、每仓总分 / 问题数 / 严重数 / 用时并按总分排序，平均分只统计已完成仓库；完成仓库写入历史，可跳回报告页
- [x] **前端单元测试**：Vitest 覆盖 store 的配置持久化 / 历史合并与迁移 / 评分口径 / 趋势聚合 / 批量调度状态机，以及忽略规则编辑器的纯函数（27 用例），并接入 CI 质量门禁
- [x] **独立设置页**：检查器开关 / 阈值 / 历史范围 / 自定义规则 / 忽略规则编辑器从连接页拆出（顶栏「设置」），连接页只保留「选仓库 → 规模预估 → 开始体检」；配置全局生效、存本机
- [x] **离线漏洞库管理**：设置页可导入 JSONL 索引并查看状态（条目数 / 更新时间 / 体积）；先验后写，损坏或空索引不会覆盖既有可用库；纯本地操作，不做网络请求

### 近期（v0.2 ~ v0.3）

- [ ] **PDF 导出**（报告含中文，需解决 CJK 字体嵌入）

### 中长期（v0.5 → v1.0）

- [ ] **漏洞库随包分发 / 自动更新**：设置页已支持导入 JSONL 索引并查看状态，但索引未随安装包分发，也不做联网自动更新（受「纯本地零网络」定位限制）
- [ ] **官方 GitHub Action**、自动更新、Homebrew / Scoop / Winget 分发
- [ ] **团队报告协作**：Report as Code，整改项 PR 评审闭环

## 参与贡献

欢迎提 issue 与 PR，完整流程、代码纪律与新增检查器的写法见 [`CONTRIBUTING.md`](CONTRIBUTING.md)（[English](CONTRIBUTING_EN.md)）。

- 遇到问题先看 [`SUPPORT.md`](SUPPORT.md)（含未签名安装、误报处理、评分口径等常见问题）
- 缺陷与功能建议：使用 [issue 模板](.github/ISSUE_TEMPLATE)
- 提交 PR：请对照 [PR 模板](.github/PULL_REQUEST_TEMPLATE.md) 的验证清单
- 社区行为准则：[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)（[English](CODE_OF_CONDUCT_EN.md)）
- 安全漏洞：请**勿**公开提交，按 [`SECURITY.md`](SECURITY.md)（[English](SECURITY_EN.md)）的私密渠道报告
- 项目治理与角色：[`GOVERNANCE.md`](GOVERNANCE.md) ｜ 贡献者名单：[`AUTHORS.md`](AUTHORS.md)
- 版本变更记录：[`CHANGELOG.md`](CHANGELOG.md)（Keep a Changelog + 语义化版本）

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源发布。