# Audie 0.2.1 Preview

## English

Audie 0.2.1 is the first downloadable Windows preview and also includes the latest macOS Apple Silicon build.

### What's new

- Added a Windows x64 preview installer
- Set the Windows default shortcut to `Ctrl+Shift+Space`
- Added Windows shortcut recording, conflict handling, and migration from legacy `Fn` settings
- Show the configured shortcut consistently on the Home, setup, and Settings screens
- Added rotating local diagnostic logs without storing complete transcripts or recordings
- Added a branded Windows installer with Simplified Chinese and English language selection
- Added Windows CI and release packaging

### Windows installation

Download `Audie_0.2.1_x64-setup.exe`, choose the installer language, and follow the setup wizard.

This preview is not code-signed, so Windows SmartScreen may display a “Windows protected your PC” warning and list the publisher as unknown. If you downloaded the installer from the official Audie GitHub release, select **More info**, confirm the app name is `Audie_0.2.1_x64-setup.exe`, and then select **Run anyway**. Do not disable SmartScreen; delete the installer if its source or file name is unexpected.

### macOS installation

Download the Apple Silicon DMG, drag Audie into Applications, then run:

```bash
xattr -dr com.apple.quarantine /Applications/Audie.app
open /Applications/Audie.app
```

This preview is not notarized by Apple. Updating may require granting permissions again.

## 中文

Audie 0.2.1 是首个可下载的 Windows 预览版，同时包含最新的 macOS Apple Silicon 版本。

### 新增内容

- 增加 Windows x64 预览版安装包
- 将 Windows 默认快捷键设为 `Ctrl+Shift+Space`
- 修复 Windows 快捷键录制、冲突处理及旧版 `Fn` 配置迁移
- 在首页、引导页和设置页统一显示当前快捷键
- 增加自动轮转的本地诊断日志，不保存完整转写文本或录音
- 增加带有 Audie 品牌素材的 Windows 安装器，支持简体中文和英语选择
- 增加 Windows CI 和发版构建

### Windows 安装

下载 `Audie_0.2.1_x64-setup.exe`，选择安装器语言，然后按照安装向导完成安装。

当前预览版没有代码签名，因此 Windows SmartScreen 可能显示“Windows 已保护你的电脑”，发布者也会显示为未知。如果安装包来自 Audie 官方 GitHub Release，请点击“更多信息”，确认应用名称为 `Audie_0.2.1_x64-setup.exe`，然后点击“仍要运行”。不需要关闭 SmartScreen；如果安装包来源或文件名不符合预期，请直接删除。

### macOS 安装

下载 Apple Silicon DMG，将 Audie 拖入“应用程序”，然后运行：

```bash
xattr -dr com.apple.quarantine /Applications/Audie.app
open /Applications/Audie.app
```

当前预览版未经 Apple 公证，更新后可能需要重新授予系统权限。
