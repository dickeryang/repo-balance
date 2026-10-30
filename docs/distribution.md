# 分发渠道与发布流程

> 本文是发布与分发的操作手册：应用内更新、包管理器清单、Release notes 三件事的
> 生成方式、密钥管理与人工步骤。**代码侧的自动化边界写清楚**，避免出现「文档说
> 自动、实际要手动」的落差。

---

## 〇、全貌

```
git tag v0.5.0 → push tag
        │
        ├─ build（三平台）──→ Release 资产：dmg / setup.exe / deb / AppImage / 便携 zip
        │                    └─ 配置了签名私钥时：.sig + latest.json（应用内更新的数据源）
        │
        ├─ manifests ───────→ 从资产算 SHA-256 → Homebrew Cask / Scoop / WinGet 清单
        │                    （作为 Release 附件回传，不代推第三方仓库）
        │
        └─ Release 正文 ────→ scripts/release-notes.sh 从 CHANGELOG.md 提取
```

| 通道 | 自动化程度 | 使用者命令 |
|---|---|---|
| 应用内更新 | ✅ 打 tag 即产出签名与 `latest.json` | 设置页「检查更新」 |
| Homebrew Cask | ⚠️ 清单自动生成，推 tap 需人工 PR | `brew install --cask dickeryang/tap/repo-balance` |
| Scoop | ⚠️ 清单自动生成，推 bucket 需人工 PR | `scoop bucket add dickeryang …` |
| WinGet | ⚠️ 清单自动生成，PR 到 `microsoft/winget-pkgs` | `winget install dickeryang.RepoBalance` |

「自动生成 + 人工 PR」是刻意选择：三个包管理器各有自己的仓库、审核与凭证体系，
代持这些凭证的收益远小于风险。

---

## 一、应用内更新（`tauri-plugin-updater`）

### 行为约定

- **默认不联网**。只有两种触发方式：用户点设置页「检查更新」，或用户显式开启
  「启动时自动检查更新」（`localStorage` 键 `rb-update-auto-check`，默认关闭）。
- 请求只取版本元数据（`latest.json`）与安装包，不携带仓库内容、路径、扫描结果或
  设备标识。**没有后台轮询，没有遥测。**
- 安装包必须通过 minisign 验签（公钥内置在 `crates/rb-app/tauri.conf.json` 的
  `plugins.updater.pubkey`）；签名缺失或不匹配一律拒绝安装。
- 更新端点：`https://github.com/dickeryang/repo-balance/releases/latest/download/latest.json`
  （GitHub 不可达时，用户仍可从 Release 页手动下载安装包；AtomGit 侧目前只做源码托管）。

### 密钥管理

签名密钥对已生成在本机：

| 文件 | 用途 | 是否入库 |
|---|---|---|
| `~/.tauri/repo-balance.key` | 私钥，签名安装包 | ❌ **绝不入库**，需离线备份 |
| `~/.tauri/repo-balance.key.pub` | 公钥 | ✅ 已内联到 `tauri.conf.json` |

**首次配置 CI（两个平台各配一次）**：

```bash
# GitHub
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/repo-balance.key
# 私钥无密码时，这个 secret 留空即可（不要写成空字符串以外的内容）
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --body ""

# AtomGit：仓库 → 设置 → CI/CD 变量，同上两项
```

未配置私钥时流水线**不会失败**：只打印一条 warning，跳过 `.sig` 与 `latest.json`，
其余安装包照常发布（此时应用内检查更新会提示失败，需手动下载）。

**轮换密钥的正确顺序**（顺序错了老用户会收不到更新）：

1. 先用旧钥发布一个「带新公钥」的版本（客户端仍用旧钥验签，能装上）；
2. 再切换到新私钥发布后续版本——此时已升级的客户端用新公钥验签。

直接换公钥会让所有未升级的客户端拒绝安装新版本。

### 打 tag 后的验收

Release 资产里应包含：

- [ ] `RepoBalance_<ver>_universal.dmg`（macOS 通用包）
- [ ] `RepoBalance_<ver>_x64-setup.exe`（Windows NSIS 安装器）
- [ ] `RepoBalance_<ver>_windows-x64-portable.zip`（Scoop 用便携包）
- [ ] `RepoBalance_<ver>_amd64.deb` / `_amd64.AppImage`（Linux）
- [ ] `*.sig`（每个平台的更新包签名）
- [ ] `latest.json`（应用内更新的清单）
- [ ] `SHA256SUMS.txt` + `repo-balance.rb` / `repo-balance.json` / 三个 winget yaml

缺 `latest.json` 或 `.sig` 说明签名 secret 没配好（流水线会给出 warning）。

---

## 二、包管理器清单

生成器：`scripts/gen-packaging-manifests.sh`

```bash
# 本地演练（用真实发布资产的校验和）
sha256sum RepoBalance_0.5.0_universal.dmg \
          RepoBalance_0.5.0_x64-setup.exe \
          RepoBalance_0.5.0_windows-x64-portable.zip > SHA256SUMS
bash scripts/gen-packaging-manifests.sh 0.5.0 SHA256SUMS dist/manifests
```

产物：

```
dist/manifests/
├── homebrew/repo-balance.rb
├── scoop/repo-balance.json
└── winget/dickeryang.RepoBalance/0.5.0/
    ├── dickeryang.RepoBalance.yaml
    ├── dickeryang.RepoBalance.installer.yaml
    └── dickeryang.RepoBalance.locale.en-US.yaml
```

要求校验和来自**真实产物**：缺 dmg / setup.exe / 便携 zip 任一即报错退出，不会生成
指向不存在文件的清单。

### Homebrew（Cask）

1. 新建 tap 仓库 `dickeryang/homebrew-tap`（命名必须是 `homebrew-*`）；
2. 把 `repo-balance.rb` 放到 `Casks/repo-balance.rb`；
3. 用户安装：

```bash
brew install --cask dickeryang/tap/repo-balance
# 未签名应用首次启动被 Gatekeeper 拦下时：
xattr -dr com.apple.quarantine "/Applications/RepoBalance.app"
```

Cask 的 `caveats` 已写明放行命令；不做公证（notarization）前不要去掉它。

### Scoop（bucket）

1. 新建 bucket 仓库 `dickeryang/scoop-bucket`；
2. 把 `repo-balance.json` 放到 `bucket/repo-balance.json`；
3. 用户安装：

```powershell
scoop bucket add dickeryang https://github.com/dickeryang/scoop-bucket
scoop install dickeryang/repo-balance
```

清单指向**便携 zip**（`bin` 直接指向 `RepoBalance.exe`），避免静默运行 NSIS 安装器；
Windows 10/11 自带 WebView2，旧系统需先装 Evergreen Runtime（已写进 `notes`）。

### WinGet

1. Fork `microsoft/winget-pkgs`；
2. 把三个 yaml 放到 `manifests/d/dickeryang/RepoBalance/<version>/`；
3. 本地校验后提 PR：

```powershell
winget validate manifests/d/dickeryang/RepoBalance/0.5.0
winget install --manifest manifests/d/dickeryang/RepoBalance/0.5.0
```

`InstallerType: nullsoft` + `Scope: user` + `InstallerSwitches.Custom: /S`，与 Tauri
NSIS 安装器的行为一致（Tauri 默认按用户安装，不需要管理员权限）。

---

## 三、Release notes

正文来源是 `CHANGELOG.md`，由 `scripts/release-notes.sh` 提取：

```bash
bash scripts/release-notes.sh v0.5.0            # 打印到 stdout
bash scripts/release-notes.sh 0.5.0 notes.md   # 写入文件
```

约定：

- 发布前必须先在 CHANGELOG 写好 `## [0.5.0] - YYYY-MM-DD` 小节——脚本找不到就
  退出码 1，流水线回退到通用文案并保留 warning，**不会**发出一份空的更新说明；
- Release notes 与 CHANGELOG 是同一份事实，不另外维护第二份文案；
- `.github/release.yml` 只在「忘记补 CHANGELOG、改用 GitHub 自动生成」时兜底，
  按标签分类（新功能 / 缺陷修复 / 文档 / 构建 CI）。

---

## 四、需要配置的 Secrets

| 名称 | 用途 | 缺失后果 |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | 安装包签名（minisign 私钥全文） | 无 `.sig` / `latest.json`，应用内更新不可用 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 私钥口令（无口令留空） | 有口令却未配置 → 签名失败 |
| `GITHUB_TOKEN` | 内置，上传 Release 资产 | — |

AtomGit 流水线（`ci/atomgit-pipeline.yml`）目前只跑质量门禁与体检，不参与打包；
打包与分发以 GitHub Actions 为准（`build-desktop.yml`）。

---

## 五、已知边界

- **不做后台静默更新**：没有定时轮询，也不会在用户不知情时下载安装包——这是
  「默认零网络」定位的代价，换来的是可审计的出站行为。
- **不做代码签名 / 公证**：macOS 需要 `xattr` 放行，Windows 首次运行会有 SmartScreen
  提示；两者都在 README「安装体验」与 Cask `caveats` 中如实写明。
- **不代推第三方仓库**：tap / bucket / winget-pkgs 的 PR 由维护者手动提交，仓库里
  只保证清单是「由真实资产校验和生成」的。
