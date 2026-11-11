# Proposal

## Why

回到作者已批准的[整体整理计划](../refactor-module-organization/design.md)：先剥离 lib.rs 与 commands.rs 的内联测试，便于阅读业务及后续按职责拆分。恢复修复已单独合并 main，本片不再修改其功能。

## What Changes

- 两个原 tests 模块改为外部单元测试模块，分别放 tests/lib_tests.rs、tests/commands_tests.rs。
- 子测试按 ASR 配置、润色配置、录音缓存/流式、设置、provider、快捷键与凭据职责分文件，每个少于 400 行。
- 保留测试属性、断言、私有访问与现有恢复测试；生产代码逐字保持，只有 tests 模块声明变化。
- 记录搬迁前后测试身份、生产代码与测试正文一致性；更新进度，不提前标记全部架构整理完成。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

无；skip_specs: true，纯测试文件组织调整。

## Impact

src-tauri/src/lib.rs、commands.rs 的测试声明；src-tauri/src/tests/{lib_tests.rs,commands_tests.rs,lib/,commands/}。不改前端、依赖、配置、可见性或业务逻辑；现有 recovery 测试目录保持不动。
