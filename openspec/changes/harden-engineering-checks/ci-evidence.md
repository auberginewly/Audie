# CI 故障与修复

## RED：实际失败

基线 8bd115d 的 [CI 42973172954](https://github.com/auberginewly/Audie/actions/runs/42973172954)：Windows 前端 build、cargo build 成功；cargo test 构建完成后、尚未执行测试便退出 0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND。macOS job 取消且未执行检查。

## 根因与最小修复

- Windows：Tauri 2.6.2 / tauri-winres 0.3.6 / embed-resource 3.0.9 的默认资源仅链接 bin，lib 单测 exe 缺 Common Controls v6 清单。Tauri 导入 TaskDialogIndirect，系统使用旧 comctl32 时入口点不存在。参考 [Tauri 维护者方案](https://github.com/tauri-apps/tauri/issues/13419#issuecomment-3398457618)：仅 Windows MSVC 关闭 Tauri 自嵌入清单，使用通用 rustc-link-arg 将清单嵌入所有目标；保留图标与版本资源。XML 与安装的 Tauri 默认清单逐字一致。未使用不覆盖 libtests 的 rustc-link-arg-tests，也未升级依赖或加测试 runner。Handy build.rs 只调用默认 Tauri 构建，不能单独解决该上游测试缺口。
- macOS：GitHub Actions 的 [macOS 14 镜像已于 2026-11-02 退役](https://github.com/actions/runner-images/issues/15226)，当前 [官方标签表](https://github.com/actions/runner-images#available-images) 中 macos-15 为 Apple Silicon。CI quality、release validate/build 三处一起迁移到 macos-15；release 仍只在 release 分支触发，本次不发版。

本地 cargo check --locked 通过；独立方案/质量审查通过。构建修复独立提交 d6eea84 并推送 main，[远端验证 42976867549](https://github.com/auberginewly/Audie/actions/runs/42976867549) 已触发，结果随后追加。CI 通过不代表 Windows 热键/剪贴板/凭据系统已真机验收。

工程检查接到 CI quality 和 release validate，阻止超长源文件及维护公共文档格式/链接错误进入主线与发布。

## GREEN：远端实际通过

[42976867549](https://github.com/auberginewly/Audie/actions/runs/42976867549) 在 d6eea84 完成且 conclusion=success：Windows 前端 build、cargo build/test/clippy 全部成功；macOS quality 全部成功。最终隐私与工程检查集成后的新提交还会单独等待 CI。

## 整合代码验证

[42979405201](https://github.com/auberginewly/Audie/actions/runs/42979405201) 对 main=abb12dd 完成且 success。Windows 前端 build、cargo build/test/clippy 成功；macOS quality 包括新增 check:engineering、前端测试/静态检查/build 和 Rust fmt/test/clippy 全部成功。该轮包含隐私修复、规模/指南检查和 pipeline 可读性改动。收尾仅更新进度与证据，不改生产代码。
