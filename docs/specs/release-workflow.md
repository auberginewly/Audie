# Audie 发布流程

## 分支职责

- `main`：日常开发，保持可构建、可演示。
- `release`：发布入口，只接受 `main -> release` PR，不直接提交。
- 不建立长期 `beta` 或 `preview` 分支；测试版本使用 GitHub Pre-release 表达。

## 版本来源

每次发版同时更新以下三处，应用设置页会从 Tauri 元数据自动显示版本：

```text
package.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
```

当前三处版本均为 **0.2.1**，下一版本号尚未决定；命名约定见 [VERSIONING.md](../VERSIONING.md)。运行 `node scripts/check-release-version.mjs` 检查三处版本和双语说明；发布前先同步远端 tags，再运行 `node scripts/check-release-version.mjs --require-unreleased`，额外确认本地对应 tag 尚不存在。脚本仅接受无预发布后缀的版本。

历史 [v0.2.1](https://github.com/auberginewly/Audie/releases/tag/v0.2.1) 已有 Apple Silicon DMG 与 Windows x64 NSIS 产物，GitHub prerelease 标记为 false。以下描述当前 [release.yml](../../.github/workflows/release.yml) 对未来版本的行为；本地重打 0.2.1 测试包不构成新发布，也不覆盖已有 tag/产物。

## 发布步骤

1. 在 `main` 更新版本和 `RELEASE_NOTES.md`，说明保持 English 在上、中文在下。
2. 完成 CI 与本机核心听写闭环验收。
3. 创建 `main -> release` PR，标题使用 `release: vX.Y.Z`。
4. 合并触发 `release` 分支 push；工作流校验版本与工程检查，构建 Apple Silicon DMG 和 Windows x64 NSIS。DMG 挂载后校验版本、arm64 与签名完整性；Windows 安装包校验名称、版本与无签名状态，并计算 SHA256。两平台构建通过后，创建对应 tag 和 GitHub Pre-release。
5. 从 GitHub 下载 DMG，拖入 Applications 后执行：

```bash
xattr -dr com.apple.quarantine /Applications/Audie.app
open /Applications/Audie.app
```

6. 在 macOS 安装产物上再次验收授权、快捷键、录音、转写、润色、注入和 Keychain；Windows 安装包另在真实 Windows 上验收。构建、单测与 CI 不代替系统行为验收。

## 当前边界

- macOS 当前使用 ad-hoc 签名，未经 Apple 公证，只面向早期测试和技术用户。Windows 当前为 unsigned NSIS 预览安装包。
- 打包必须显式传入 `--config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'`，确保 `.app` 整体完成 ad-hoc 签名和资源封装校验。
- `xattr` 只能作用于 `/Applications/Audie.app`；禁止指导用户关闭整机 Gatekeeper。
- 发布过的版本与 tag 不覆盖。修复使用新 PATCH 版本，功能使用新 MINOR 版本。
- 当前工作流没有 Developer ID 签名、公证或 Tauri updater。正式公众分发所需能力另开切片确认，不由本次文档核对引入。

## GitHub 模板

- Bug 与 Feature 使用 `.github/ISSUE_TEMPLATE/` 中的英中双语表单。
- PR 使用 `.github/PULL_REQUEST_TEMPLATE.md`。
- Release 正文唯一来源是根目录 `RELEASE_NOTES.md`，不从 Git commit 自动生成用户说明。
