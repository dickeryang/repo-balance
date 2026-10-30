#!/usr/bin/env bash
# 由发布产物生成 Homebrew Cask / Scoop / WinGet 清单（不联网、不发布）。
#
# 用法：
#   bash scripts/gen-packaging-manifests.sh <版本> <sha256 清单> <输出目录>
#
# 例：
#   sha256sum RepoBalance_*.dmg RepoBalance_*-setup.exe RepoBalance_*portable.zip > SHA256SUMS
#   bash scripts/gen-packaging-manifests.sh 0.5.0 SHA256SUMS dist
#
# 产物（输出目录下）：
#   homebrew/repo-balance.rb                     → Homebrew tap（Casks/repo-balance.rb）
#   scoop/repo-balance.json                      → Scoop bucket
#   winget/dickeryang.RepoBalance/<ver>/*.yaml   → winget-pkgs（三个清单文件）
#
# 说明：
#   - 只生成清单，**不推送到任何仓库**：brew tap / scoop bucket / winget-pkgs 各有
#     自己的仓库与凭证，推送是维护者的一次性 PR（见 docs/distribution.md）；
#   - 校验和必须来自真实发布产物，生成器不做任何猜测；缺少任一必需产物即失败，
#     避免发出指向不存在文件的清单；
#   - 清单里的下载地址固定为 GitHub Releases（与更新端点同一来源）。
set -euo pipefail

VERSION_RAW="${1:-}"
SUMS="${2:-}"
OUT="${3:-}"

die() { echo "[manifests] 错误: $*" >&2; exit 1; }
info() { echo "[manifests] $*"; }

[[ -n "$VERSION_RAW" && -n "$SUMS" && -n "$OUT" ]] || die "用法: $0 <版本> <sha256 清单> <输出目录>"
[[ -f "$SUMS" ]] || die "找不到校验和文件: $SUMS"

VERSION="${VERSION_RAW#v}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.]+)?$ ]] || die "版本号格式不合法: $VERSION_RAW"

REPO="dickeryang/repo-balance"
BASE_URL="https://github.com/$REPO/releases/download/v$VERSION"

# 从校验和清单里挑出唯一匹配的文件名（兼容 `sha256sum` 与 `shasum -a 256` 输出）
pick() {
  local pattern="$1"
  awk -v pat="$pattern" '$2 ~ pat { print $2; exit }' "$SUMS"
}

hash_of() {
  local file="$1"
  awk -v f="$file" '$2 == f { print tolower($1); exit }' "$SUMS"
}

DMG="$(pick '_universal\.dmg$')"
[[ -n "$DMG" ]] || DMG="$(pick '\.dmg$')"
SETUP="$(pick '-setup\.exe$')"
PORTABLE="$(pick 'portable\.zip$')"

[[ -n "$DMG" ]] || die "校验和清单里没有 .dmg（macOS 安装包）"
[[ -n "$SETUP" ]] || die "校验和清单里没有 *-setup.exe（Windows NSIS 安装包）"
[[ -n "$PORTABLE" ]] || die "校验和清单里没有 *portable.zip（Windows 便携包，供 Scoop 使用）"

DMG_SHA="$(hash_of "$DMG")"
SETUP_SHA="$(hash_of "$SETUP")"
PORTABLE_SHA="$(hash_of "$PORTABLE")"
SETUP_SHA_UPPER="$(printf '%s' "$SETUP_SHA" | tr '[:lower:]' '[:upper:]')"

for pair in "DMG:$DMG_SHA" "SETUP:$SETUP_SHA" "PORTABLE:$PORTABLE_SHA"; do
  name="${pair%%:*}"
  value="${pair#*:}"
  [[ "$value" =~ ^[0-9a-f]{64}$ ]] || die "$name 的 sha256 不合法（得到 '$value'）"
done

mkdir -p "$OUT/homebrew" "$OUT/scoop" "$OUT/winget/dickeryang.RepoBalance/$VERSION"

# ---------------------------------------------------------------- Homebrew Cask
cat > "$OUT/homebrew/repo-balance.rb" <<EOF
# frozen_string_literal: true

# 由 scripts/gen-packaging-manifests.sh 生成，请勿手改。
cask "repo-balance" do
  version "$VERSION"
  sha256 "$DMG_SHA"

  url "$BASE_URL/#{version}/RepoBalance_#{version}_universal.dmg",
      verified: "github.com/$REPO/"
  name "RepoBalance"
  desc "Local-only Git repository health checkup"
  homepage "https://github.com/$REPO"

  app "RepoBalance.app"

  caveats <<~EOS
    仓衡暂未做代码签名，首次启动前需放行：
      xattr -dr com.apple.quarantine "#{appdir}/RepoBalance.app"
  EOS
end
EOF

# ---------------------------------------------------------------------- Scoop
cat > "$OUT/scoop/repo-balance.json" <<EOF
{
  "\$schema": "https://raw.githubusercontent.com/ScoopInstaller/Scoop/master/schema.json",
  "version": "$VERSION",
  "description": "Local-only Git repository health checkup",
  "homepage": "https://github.com/$REPO",
  "license": "Apache-2.0",
  "architecture": {
    "64bit": {
      "url": "$BASE_URL/RepoBalance_${VERSION}_windows-x64-portable.zip",
      "hash": "$PORTABLE_SHA"
    }
  },
  "bin": "RepoBalance.exe",
  "shortcuts": [["RepoBalance.exe", "RepoBalance"]],
  "notes": [
    "需要 WebView2 运行时（Windows 10/11 已内置，旧系统请先安装 Evergreen Runtime）。",
    "更新检查默认关闭，仅在设置页手动触发。"
  ],
  "checkver": {
    "github": "https://github.com/$REPO"
  },
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "$BASE_URL/RepoBalance_\$version_windows-x64-portable.zip"
      }
    }
  }
}
EOF

# --------------------------------------------------------------------- WinGet
cat > "$OUT/winget/dickeryang.RepoBalance/$VERSION/dickeryang.RepoBalance.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.6.0.schema.json
PackageIdentifier: dickeryang.RepoBalance
PackageVersion: $VERSION
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
EOF

cat > "$OUT/winget/dickeryang.RepoBalance/$VERSION/dickeryang.RepoBalance.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json
PackageIdentifier: dickeryang.RepoBalance
PackageVersion: $VERSION
InstallerLocale: en-US
Platform:
  - Windows.Desktop
MinimumOSVersion: 10.0.0.0
InstallerType: nullsoft
Scope: user
InstallModes:
  - silent
  - silentWithProgress
UpgradeBehavior: install
Installers:
  - Architecture: x64
    InstallerUrl: $BASE_URL/RepoBalance_${VERSION}_x64-setup.exe
    InstallerSha256: $SETUP_SHA_UPPER
    InstallerSwitches:
      Custom: /S
ManifestType: installer
ManifestVersion: 1.6.0
EOF

cat > "$OUT/winget/dickeryang.RepoBalance/$VERSION/dickeryang.RepoBalance.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.1.6.0.schema.json
PackageIdentifier: dickeryang.RepoBalance
PackageVersion: $VERSION
PackageLocale: en-US
Publisher: dickeryang
PublisherUrl: https://github.com/dickeryang
PackageName: RepoBalance
PackageUrl: https://github.com/$REPO
License: Apache-2.0
LicenseUrl: https://github.com/$REPO/blob/master/LICENSE
Copyright: Copyright (c) repo-balance contributors
ShortDescription: Local-only Git repository health checkup with five scored dimensions.
Description: RepoBalance scans a Git repository locally and scores it across five dimensions (structure, security, branches, history, dependencies). No network access during scanning, no telemetry, and evidence is masked before it leaves the Rust core.
Moniker: repobalance
Tags:
  - git
  - repository
  - health-check
  - security
  - offline
  - tauri
ReleaseNotesUrl: https://github.com/$REPO/releases/tag/v$VERSION
ManifestType: locale
ManifestVersion: 1.6.0
EOF

info "版本 $VERSION 的清单已生成到 $OUT"
info "  homebrew/repo-balance.rb"
info "  scoop/repo-balance.json"
info "  winget/dickeryang.RepoBalance/$VERSION/（3 个文件）"
