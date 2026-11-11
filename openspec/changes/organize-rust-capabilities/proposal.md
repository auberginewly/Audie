# Proposal

## Why

落实作者已批准的[整体模块组织方案](../refactor-module-organization/design.md) S6a–S6c，能力代码和测试就近，降低文件阅读负担。

## What Changes

- AudioManager 按录音会话、CPAL 采集、预览、电平与缓冲分文件，API 不变。
- HistoryManager 保留 API，SQL 按存储、查询、统计归属；SQLite 路径、表和口径不变。
- LLM 分 trait/构建、OpenAI-compatible 请求、sanitizer；provider_test 分连接与模型查询。
- ASR 先提取内联测试；豆包仍超长的 client 将连接配置和 PCM codec 归既有 config/codec。
- 所有业务及测试文件目标不超过 400 物理行；已有测试正文和属性保留。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无。skip_specs: true，内部组织调整。

## Impact

仅 src-tauri/src/managers/{audio,history}、llm、asr、provider_test 及本 change；设置消费者改 crate::settings。无新依赖，不改协议、算法、线程、超时、命令/event/配置字段。运行应用与 Windows 验收由主集成记录，不从源码搬迁推断。
