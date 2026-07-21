# Audie v0.1.0 发布流程

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

运行 `node scripts/check-release-version.mjs` 检查三处版本和双语说明；发布前运行 `node scripts/check-release-version.mjs --require-unreleased`，额外确认对应 tag 尚不存在。

## 发布步骤

1. 在 `main` 更新版本和 `RELEASE_NOTES.md`，说明保持 English 在上、中文在下。
2. 完成 CI 与本机核心听写闭环验收。
3. 创建 `main -> release` PR，标题使用 `release: vX.Y.Z`。
4. 合并后 `.github/workflows/release.yml` 构建 Apple Silicon DMG，并创建对应 tag 和 GitHub Pre-release。
5. 从 GitHub 下载 DMG，拖入 Applications 后执行：

```bash
xattr -dr com.apple.quarantine /Applications/Audie.app
open /Applications/Audie.app
```

6. 再次验收授权、快捷键、录音、转写、润色、注入和 Keychain。

## 当前边界

- v0.1.0 使用 ad-hoc 签名，未经 Apple 公证，只面向早期测试和技术用户。
- 打包必须显式传入 `--config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'`，确保 `.app` 整体完成 ad-hoc 签名和资源封装校验。
- `xattr` 只能作用于 `/Applications/Audie.app`；禁止指导用户关闭整机 Gatekeeper。
- 发布过的版本与 tag 不覆盖。修复使用新 PATCH 版本，功能使用新 MINOR 版本。
- Developer ID、Hardened Runtime、公证和 Tauri updater 留到正式公众分发阶段。

## GitHub 模板

- Bug 与 Feature 使用 `.github/ISSUE_TEMPLATE/` 中的英中双语表单。
- PR 使用 `.github/PULL_REQUEST_TEMPLATE.md`。
- Release 正文唯一来源是根目录 `RELEASE_NOTES.md`，不从 Git commit 自动生成用户说明。
