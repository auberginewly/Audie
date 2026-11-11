# Proposal

## Why

执行已批准的 [整体职责整理](../refactor-module-organization/design.md) S2–S5，让入口只组装，让作者能沿快捷键到粘贴定位代码。2026-11-11 作者明确要求全部继续完成。

## What Changes

- settings 管数据、默认值、校验、patch 与 TOML 读写；commands 按功能提供薄入口。
- hotkeys 管注册与设置录键，pipeline 管上下文、事件、配置映射、录音、处理与恢复。
- Platform 接收最小 overlay 初始化/显示/隐藏能力；AppKit、多屏跟随在 macOS 实现，执行编号判断留 pipeline。
- macOS 快捷键拆手势、键码、解析、run loop；所有剩余 Rust 内嵌测试分离且就近。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无。skip_specs，只有代码组织调整。

## Impact

src-tauri/src/{lib.rs,settings,commands,hotkeys,pipeline,platform} 及 enhance/transcription 测试落点。不增加库、状态机、provider、agent、多模板或服务器。原 command/event、配置字段、历史表、线程和恢复锁保留。
