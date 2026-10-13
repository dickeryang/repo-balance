# 贡献指南

感谢你关注「仓衡 RepoBalance」。本项目为纯本地的 Git 仓库健康度体检工具，**零网络请求、不采集任何数据**，任何改动都需守住这条底线。

## 一、先决条件

| 工具 | 版本 | 说明 |
|---|---|---|
| Rust | 1.88+ | 依赖树 MSRV 下界，见根 `Cargo.toml` 的 `rust-version` |
| Node.js | 14.18+ | Vite 4 最低要求 |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |

## 二、开发流程

```bash
# 1. 安装前端依赖（npm 需指向官方源）
cd ui && npm install --registry https://registry.npmjs.org/ && cd ..

# 2. 一体化开发（自动起 Vite devServer + Rust 热重载）
cargo tauri dev
```

提交前必须通过全部质量门禁：

```bash
cargo fmt --all -- --check        # 格式（rustfmt.toml：max_width = 100 + 字段初始化简写）
cargo clippy --all-targets --workspace   # 静态检查（零 warning 为门禁）
cargo test --workspace                   # 单元测试（含 git2 真实仓库夹具）
cd ui && npm run lint && npm run build   # 前端 ESLint + 构建
```

## 三、代码纪律

1. **分层不可越界**：`rb-core`（检查器内核）**不得**出现 `tauri` / `std::fs`；`rb-app`（壳）不得出现 `git2`。文件读取一律经 `git` 模块，前端不触碰文件系统。
2. **非测试代码禁用 `unwrap()` / `expect()`**：错误统一 `?` 向上传播。
3. **公开类型必须有完整的 `///` 文档**：说明「是什么 / 为什么 / 边界与错误」，含 `# Errors` / `# Examples` 段落。
4. **提交信息用约定式提交**：`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`，正文用中文说明动机。
5. **前端样式与文案**：样式走 CSS 变量（`--bg-card` / `--accent` / `--text-dim` 等），文案一律进 `ui/src/i18n/index.ts`，**zh/en 两个字典必须同步加键**。

## 四、新增检查器

检查器是插件式的，新增只需三步（无需改动引擎）：

1. 在 `crates/rb-core/src/checker/` 新建文件，实现 `Checker` trait：

   ```rust,ignore
   fn id(&self) -> &'static str;                       // 稳定唯一 id
   fn category(&self) -> Category;                     // 归属维度
   fn plan(&self, ctx: &ScanContext) -> ScanPlan;      // 声明数据切片（幂等）
   fn check(&self, ctx: &ScanContext) -> Vec<Finding>; // 纯计算，同输入同输出
   ```

2. 在 `checker/mod.rs` 导出并在 `src/bin/rb.rs` 与 `crates/rb-app/src/commands.rs` 注册；id 同步加入前端 `ALL_CHECKER_IDS`。
3. 补测试：覆盖 **命中 / 不命中 / 边界阈值 / 忽略规则** 四类用例，沿用中文测试名 + `TestRepo` 夹具范式。

约定：

- `check` 为纯函数，不读时钟、不做 I/O；需要「当前时间」的口径以仓库内最新提交时间为基准。
- 涉及密钥的 evidence **构造时即掩码**，一处掩码，处处掩码。
- 未注册检查器的维度评分为 `None`（前端显示 N/A），不得伪造满分。

## 五、提交 PR

- 一个 PR 只做一件事，描述里写清「动机 + 方案 + 验证方式」。
- CI（`.github/workflows/repo-balance.yml`）会对本仓库跑体检，`rb scan . --ci` 有 Critical 结论即失败，请保持零 Critical。
- 仓库同时托管在 AtomGit 与 GitHub，PR 合并后由维护者**同一提交双端推送**保持同步。

## 六、报告问题

用 issue 模板提交，并尽量提供：

- 复现仓库特征（规模、分支数、是否有 LFS 等，注意脱敏）
- 仓衡版本 / 操作系统 / 安装方式
- 体检报告 JSON（`rb scan <path> --json`）

> **安全漏洞请勿公开提交**，按 [`SECURITY.md`](SECURITY.md) 的私密渠道报告。
> 「某类密钥漏报 / 示例值误报」属规则准确性问题，普通 issue 即可。

## 七、行为准则

参与本项目即表示你同意遵守 [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)（改编自 Contributor Covenant 2.1）。
请保持友善、就事论事的技术讨论。

## 八、版本与发布

显著变更记录在 [`CHANGELOG.md`](CHANGELOG.md)（Keep a Changelog 格式 + 语义化版本）。
发布流程：合并到 `master` → 打 `vX.Y.Z` tag → 触发打包 workflow 产出各平台安装包 → 双端推送 tag。
