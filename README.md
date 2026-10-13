# 仓衡 RepoBalance

[![AtomGit G-Star](https://atomgit.com/dickeryang/repo-balance/star/new_badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Star](https://atomgit.com/dickeryang/repo-balance/star/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Fork](https://atomgit.com/dickeryang/repo-balance/fork/badge.svg)](https://atomgit.com/dickeryang/repo-balance)
[![AtomGit Download](https://atomgit.com/dickeryang/repo-balance/download/badge.svg)](https://atomgit.com/dickeryang/repo-balance/releases)

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
| Rust | 1.77+ | `rustup update stable` |
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

```bash
# Rust 静态检查（零 warning）
cargo clippy --all-targets --workspace

# Rust 测试
cargo test --workspace

# 前端构建
cd ui && npm run build
```

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

## 项目结构

```
repo-balance/
├── crates/
│   ├── rb-core/              # 检查器内核（零 tauri 依赖）
│   │   ├── src/
│   │   │   ├── checker/      # Checker trait + 注册表 + 切片计划 + 上下文 + 五个内置检查器
│   │   │   ├── engine/       # 扫描编排（plan → merge → collect → check）
│   │   │   ├── git/          # git2 只读门面（类型不逃逸）
│   │   │   ├── model/        # 数据模型（RepoSnapshot/Finding/ScanReport）
│   │   │   └── bin/rb.rs     # CLI 入口
│   │   └── build.rs          # Windows advapi32 链接补齐
│   └── rb-app/              # Tauri 2 桌面壳
│       ├── src/
│       │   ├── main.rs       # Tauri Builder 入口
│       │   ├── commands.rs   # IPC 命令（5 条）
│       │   ├── dto.rs        # camelCase DTO 镜像层
│       │   ├── error.rs      # 错误映射
│       │   └── state.rs      # 扫描状态管理
│       ├── capabilities/     # 权限最小集
│       ├── icons/            # 应用图标
│       ├── tauri.conf.json   # Tauri 配置（CSP/窗口/打包）
│       └── build.rs          # tauri-build
├── ui/                       # Vue 3 前端工程
│   ├── src/
│   │   ├── views/            # 三视图（接入/扫描/报告）
│   │   ├── components/       # 通用组件（TopBar/RadarChart）
│   │   ├── stores/           # Pinia 状态管理
│   │   ├── ipc/              # IPC 封装（命令+事件）
│   │   ├── styles/           # 主题样式（14 CSS 变量）
│   │   └── types/            # TS 类型镜像
│   ├── package.json
│   └── vite.config.ts
├── docs/prototype/           # 产品原型 HTML
├── scripts/                  # 夹具脚本
└── Cargo.toml                # workspace 根
```

## 安全设计

- **CSP**: `default-src 'self'`（禁止外部资源加载）
- **权限最小集**: `core:default` + `dialog:allow-open` + `core:event:default`（无 fs/shell）
- **路径来源**: 仅系统目录选择对话框，无自由文本输入
- **纯本地零网络**: 不发送任何数据到远程服务器
- **evidence 掩码**: 证据在 rb-core 构造时掩码，导出/IPC 不复原
- **rb-core 零 tauri 依赖**: 内核可独立编译为 CLI，不耦合 UI 框架

## 规划

> 完整路线图（含现状盘点、阶段目标与验收标准）见 [`docs/roadmap.md`](docs/roadmap.md)。
> 以下仅为概要；未完成的功能一律标记为未完成，文档不超前于实现。

### 当前版本已完成（v0.1.x）

- [x] **五个检查器全部落地**：大文件（结构）、敏感信息残留（安全）、僵尸分支（分支）、提交习惯（历史）、依赖健康（依赖），五维均有真实结论
- [x] **敏感信息检查器**：私钥头 / AWS Access Key / GitHub·GitLab token / `.env` 赋值 / 高熵密钥赋值双通道（正则 + 香农熵），evidence 构造时即掩码，内置示例值停用词白名单
- [x] **僵尸分支检查器**：已合并未删除、超 N 天未活跃（默认 90 天，可配）、无远端追踪
- [x] **提交习惯检查器**：超大提交（>50 文件 Warning / >200 Critical）、低质提交信息占比、深夜提交比例（按提交时区换算）
- [x] **依赖健康检查器（轻量版）**：锁定文件缺失 / 未提交 / 被删、同生态多包管理器混用（纯结构检查，漏洞库见路线图中期任务）
- [x] **评分口径修复**：无检查器覆盖的维度显示 N/A 而非 100，总分 = 已覆盖维度平均
- [x] **报告导出**：JSON / Markdown / HTML（自包含，内联 SVG 雷达图）
- [x] **扫描历史**：本地持久化，支持两次扫描结果对比（新增/消除的问题项）
- [x] **忽略规则**：`.repobalance-ignore`，含否定规则（`!pattern`）；内置默认忽略构建产物目录（`target/`、`node_modules/` 等，可 `!` 否定）
- [x] **CI 集成**：GitHub Actions 流水线，CLI 模式跑体检并上传报告
- [x] **国际化**：界面中英文切换（i18n）

### 近期（v0.2 ~ v0.3）

- [ ] **历史持久化迁移**：localStorage → 应用数据目录文件，支持导出
- [ ] **PDF 导出**

### 中长期（v0.5 → v1.0）

- [ ] **依赖漏洞检测**：集成 OSV 离线库，一个工具检测全栈依赖漏洞
- [ ] **远程仓库体检**：URL 浅克隆后本地扫描
- [ ] **多仓库批量体检**：仓库队列 + 汇总看板，面向团队仓库治理
- [ ] **健康度趋势分析**：基于历史数据绘制质量演进曲线
- [ ] **自定义规则**：JSON 配置驱动，后续演进为 WASM 沙箱插件机制
- [ ] **官方 GitHub Action**、自动更新、Homebrew / Scoop / Winget 分发
- [ ] **团队报告协作**：Report as Code，整改项 PR 评审闭环

## 参与贡献

欢迎提 issue 与 PR，完整流程、代码纪律与新增检查器的写法见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

- 缺陷与功能建议：使用 [issue 模板](.github/ISSUE_TEMPLATE)
- 提交 PR：请对照 [PR 模板](.github/PULL_REQUEST_TEMPLATE.md) 的验证清单

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源发布。