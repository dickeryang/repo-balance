# 仓衡 RepoBalance

用华为云码道 CodeArts 代码智能体独立开发一个跨平台桌面应用「仓衡 · Git 仓库健康度体检台」，对本地 Git 仓库做结构化健康体检：大文件、僵尸分支、提交习惯、依赖健康、敏感信息残留扫描，输出可导出的体检报告。

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
2. `cargo build --release -p rb-app`（Rust 编译优化）
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
cargo build --release -p rb-app
```

产出 `target/release/rb-app.exe`（Windows），需配合 `ui/dist/` 前端资源运行。

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
│   │   │   ├── checker/      # Checker trait + 注册表 + 切片计划 + 上下文
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

### 近期规划

- [x] **报告导出增强**：支持导出 PDF / HTML 格式报告，附带雷达图快照
- [x] **检查器可配置**：允许用户启用/禁用单项检查器并调整阈值（如大文件体积上限）
- [x] **扫描历史**：本地持久化历史体检结果，支持两次扫描结果对比（新增/消除的问题项）
- [x] **忽略规则**：支持 `.repobalance-ignore` 配置文件，排除指定路径/分支不参与体检
- [x] **CI 集成**：提供 GitHub Actions / AtomGo 流水线示例，在 CI 中以 CLI 模式跑体检并输出结论摘要
- [x] **国际化**：界面中英文切换（i18n）

### 长期规划

- [ ] **远程仓库体检**：接入托管平台 API（AtomGit/GitHub/GitLab），对远程仓库只读拉取后体检
- [ ] **多仓库批量体检**：仓库队列扫描 + 汇总对比看板，适合团队仓库治理
- [ ] **趋势分析**：基于历史扫描数据绘制健康度趋势曲线，可视化仓库质量演进
- [ ] **自定义检查器插件**：开放 Checker 插件机制，允许以动态库或脚本形式扩展检查项
- [ ] **依赖漏洞数据库**：集成离线漏洞库（OSV/RustSec），依赖健康检查升级为漏洞检测
- [ ] **团队协作报告**：报告云端分享（可选、加密），支持团队评审与整改跟踪

## 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源发布。