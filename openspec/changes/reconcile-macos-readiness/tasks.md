# macOS Readiness Implementation Plan

> **For agentic workers:** 使用既有隔离工作区，按已批准范围逐项执行、逐项验证，不再次询问执行方式。

**Goal:** 清理当前文档漂移并给现有 macOS 包与核心模式留下真实验证记录。

**Architecture:** 保持现有 Manager/Command-Event/Platform/provider 与全部产品行为；仅文档修改和本地验证。

**Tech Stack:** 既有 pnpm、Cargo、OpenSpec、Tauri、macOS hdiutil/codesign/Accessibility。

## 1. 基线与方案

- [x] 1.1 记录 d8ce285、main/worktree 干净、双平台远端 CI success 和已公开 0.2.1 产物。
- [x] 1.2 运行 pnpm check:engineering、pnpm test、cargo test --manifest-path src-tauri/Cargo.toml，记录真实结果。

## 2. 文档现状

- [x] 2.1 对照 settings/catalog、release.yml、RELEASE_NOTES 与 Git commit，修正 STATUS 当前段，保留附录历史正文。
- [x] 2.2 修复本机 ROADMAP 的发布入口，更新 VERSIONING 与 release-workflow 的当前口径；版本与用户产品取舍不改。
- [x] 2.3 用现有 check:engineering 检查公共规范，并对 VERSIONING、发布流程、本机 ROADMAP 显式校验格式/链接；STATUS 仅整理现状段并单独检查相对链接，保留历史附录原文；openspec validate reconcile-macos-readiness --strict 与 git diff --check 通过。

## 3. macOS 现有包

- [x] 3.1 执行 pnpm tauri build --target aarch64-apple-darwin --bundles dmg --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'，记录退出码和实际产物。
- [x] 3.2 hdiutil verify 并只读挂载 DMG，核对 Info.plist 0.2.1、arm64 与 codesign --verify --deep --strict；复制到临时安装目录并重复验证，保留真实路径/摘要。
- [ ] 3.3 检查现有 /Applications 版本与新包，真实启动/重启和配置保留可在隔离副本核对的就执行；需作者授权/真实凭据的场景写明，不覆盖用户安装或重置 TCC。

## 4. 核心模式

- [x] 4.1 作者配合真实润色；核对 enhancing 日志/终态与临时目标结果，不以纯 ASR 通过替代。
- [x] 4.2 作者配合选区改写与独立快捷键写作；核对实际模式/目标结果。
- [x] 4.3 对处理中取消/失败重试只记录真实可执行的案例，未覆盖保持开放；不重新强制用户已免除的 toast 四态全量走查。
  - [x] 4.3.1 写作处理中取消，目标保持不变；撤销后 mode 保留为 compose，模型结果与目标一致。
  - [x] 4.3.2 真实转写失败后重试，核对保留录音、原改写选区和目标实际替换。

## 5. 收尾

- [x] 5.1 readiness-evidence 与 STATUS/任务记录一致，检查 doc diff 无生产代码/版本/密钥改动。
- [x] 5.2 既有 pre-commit 通过后独立提交，fast-forward 合入 main 并推送；真实未验收项保持开放。
