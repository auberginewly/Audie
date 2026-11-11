# Proposal

## Why

作者要求连续完成上轮列出的 CI、原生验收、诊断隐私、工程检查及核心流程可读性改进。基线 8bd115d，沿用既定架构和现有依赖。

## What Changes

- 定位 Windows 测试启动 0xc0000139 与 macOS job 取消，依据证据修正构建/runner 接线，不跳过测试。
- provider 错误及前端诊断不透传正文、凭据/URL 或完整 payload；补可复现回归。
- 用已有 Node/Prettier 加文件规模、公共文档格式/链接检查与可显式安装的提交钩子，接入现有质量 CI。
- pipeline 使用中文职责/约束说明和显式导入，不改状态机、锁或数据流；实际原生验收分项记录。

## Capabilities

### New Capabilities

- diagnostic-privacy：诊断不暴露用户内容/凭据，保持可操作的错误分类与安全上下文。

### Modified Capabilities

无其他产品能力变化。内部检查不新增运行时层。

## Impact

src-tauri/build.rs、必要 Windows manifest、CI、scripts/package scripts、新工程测试；Rust provider/error 与前端诊断调用；pipeline 注释/导入；CONTRIBUTING/ENGINEERING/STATUS 和验收证据。无新依赖、agent、多模板、协议或 provider 能力。
