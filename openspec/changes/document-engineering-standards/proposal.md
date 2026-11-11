# Proposal

## Why

作者希望补齐可参考、可执行的工程规范，已确认新增贡献指南与工程细则的方向。

## What Changes

- CONTRIBUTING：公共开发、检查与提交流程；ENGINEERING：命名、注释、接口、错误日志、生命周期和验收矩阵。
- README 入口、ARCHITECTURE 依赖规则、STATUS 与 OpenSpec 公共阅读入口同步。
- 本机忽略的 AGENTS/CLAUDE 修正旧持久化、Manager/provider 接口与 CI 描述，链接公共规范；不公开私有协作文件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无。纯文档，skip_specs: true。

## Impact

仅 Markdown 和 OpenSpec 文档配置；不修改运行时、依赖、CI、hook 或测试实现。已有英文注释不全仓改写；不改变产品/架构与历史验收边界。
