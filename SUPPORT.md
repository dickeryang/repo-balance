# 获取帮助

先在这里找答案，找不到再开 issue——这样能省下双方的时间。

## 一、先看文档

| 你想做的事 | 看哪里 |
|---|---|
| 安装、构建、跑起来 | [README](README.md)（[English](README_EN.md)）「前置环境」「开发模式」「打包发布」 |
| 在 CI 里用仓衡做门禁 | [`docs/ci-integration.md`](docs/ci-integration.md) |
| 知道接下来要做什么功能 | [`docs/roadmap.md`](docs/roadmap.md) |
| 贡献代码 / 新增检查器 | [`CONTRIBUTING.md`](CONTRIBUTING.md)（[English](CONTRIBUTING_EN.md)） |
| 版本改了什么 | [`CHANGELOG.md`](CHANGELOG.md) |

## 二、常见问题

**Q：应用能联网吗？会上传我的仓库内容吗？**
不会。仓衡无任何 HTTP 客户端，CSP 为 `default-src 'self'`，权限不含 `fs`/`shell`；扫描只读。详见 [SECURITY.md](SECURITY.md) 的「设计层面的安全边界」。

**Q：macOS 提示「无法打开，因为 Apple 无法检查其是否包含恶意软件」，Windows 出现 SmartScreen 警告？**
当前发布包**未做代码签名**（开源项目常见）。macOS：右键应用 → 打开，或执行 `xattr -dr com.apple.quarantine /Applications/仓衡.app`；Windows：点「更多信息」→「仍要运行」。介意安全风险可自行从源码构建。

**Q：某个大文件 / 某个目录不该被算进来。**
在仓库根目录新建 `.repobalance-ignore`（语法同 `.gitignore`，支持 `!` 否定）。构建产物目录（`target/`、`node_modules/` 等）已默认忽略。

**Q：某个密钥应该被检出但没检出 / 明明是示例值却被报出来。**
属规则准确性问题，请开 issue 并附上最小复现（脱敏后），我会调整规则或停用词白名单；**不要**为此走私密安全渠道。

**Q：扫描很慢或卡住。**
先确认仓库规模（提交数、文件数）；当前版本对超大仓库的历史遍历尚无时间窗限制（见 roadmap「大仓库性能」）。扫描可随时取消，会给出部分报告。

**Q：「总分」是怎么算的？为什么某维度显示 N/A？**
总分 = **已覆盖维度**的平均分；没有注册检查器的维度显示 N/A，而不是假满分。各 finding 按严重度扣分（Critical −15 / Warning −8 / Info −3），单维度下限 0。

## 三、开 issue

- 缺陷 → [bug 模板](.github/ISSUE_TEMPLATE/bug_report.yml)
- 功能建议 → [feature 模板](.github/ISSUE_TEMPLATE/feature_request.yml)
- 安全漏洞 → **请勿公开提交**，按 [SECURITY.md](SECURITY.md) 的私密渠道报告

提问时请附：仓衡版本、操作系统与架构、复现步骤、以及（如适用）`rb scan <路径> --json` 的报告片段。

## 四、参与开发

欢迎 PR，流程见 [CONTRIBUTING.md](CONTRIBUTING.md)。本项目为个人维护，响应以尽力而为为原则。
