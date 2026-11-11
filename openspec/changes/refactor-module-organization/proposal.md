# Proposal

## Why

Audie 的听写闭环已可用，但入口、设置与测试混放，功能相关的前端代码又分散在 components、hooks、types 和 store；作者难以定位规则并安全修改。通过职责归类、模块就近组织和测试分离，让现有项目更容易理解和维护，优先在 macOS 上逐片验证。

## What Changes

- 作者已取消多模板管理和循环键；保留原三模式各一份提示词，先完成独立恢复修复后再执行架构调整。

- Rust 保留 Manager + Command/Event + Platform/Provider，lib.rs 回归应用组装；拆出听写协调、快捷键接线、设置规则、按功能分类的 command 和平台窗口实现。
- 前端按 dictation、history、home、providers、text、hotkeys、audio、permissions、settings、onboarding 收拢已有组件、hook、schema、状态镜像及测试；通用 UI 和应用外壳独立。
- 业务文件目标不超过 400 个物理行；按职责拆分，超出须在当片记录原因并由作者确认，不通过删注释、压行或过度转发达标。
- Rust 单元测试移至所属模块的 tests.rs / tests/；前端测试移至功能内 __tests__/。保留现有 cargo test 与 Node test runner，迁移时同步测试发现路径和现有快捷脚本。
- 按测试分离、设置与命令、平台窗口、听写与快捷键、前端各功能分片推进；本文为总体设计，每个实际实施片建立独立关联 change，引用本设计并列清实际文件；不直接全量 apply 本清单。

## Capabilities

### New Capabilities

无。本变更只调整内部组织，不增加 agent、工具调用、语音执行或其他产品能力。

### Modified Capabilities

无。命令名称/参数、事件与序列化字段、设置文件、历史数据库、快捷键行为和三模式听写语义保持兼容；.openspec.yaml 设置 skip_specs: true，不为纯重构虚构产品需求。现有 PROJECT_SPEC.md 和进行中变更仍提供行为依据。

## Impact

| 范围 | 现有位置 | 变化及原因 |
| --- | --- | --- |
| 应用与听写 | src-tauri/src/lib.rs、pipeline.rs、state.rs | 将流程、上下文和窗口实现分开，保留初始化与状态机职责 |
| 设置与调用入口 | commands.rs、provider_test.rs | 设置规则集中，command 按功能转发，避免设置代码反向调用根模块 |
| 现有能力与平台 | managers/、llm/、platform/ | 仅拆职责过多的大文件及独立测试；macOS overlay 收回 Platform 边界，保留 Windows 接口与 CI |
| 前端功能 | src/components/Settings、Overlay、screens；hooks、types、store；App.tsx、overlay.tsx | 迁至 src/features/，更新消费者，保持展示与状态镜像不变 |
| 测试与工具 | tests/*.test.mjs、package.json、tsconfig、ESLint/Prettier、CI | 修复路径、测试发现和覆盖范围，不引入测试框架或新依赖 |
| 文档 | docs/STATUS.md、PROJECT_SPEC.md、AGENTS.md/CLAUDE.md | 实施每片时记录结果及模块位置；不把规划状态写成已完成 |

本次请求仅产出规划文件。实施前必须保护工作区已有恢复等改动，建立可回退基线，不捆绑其他功能提交，不覆盖旧验收边界。目录、迁移映射、依赖与验收见 [design.md](design.md)，分片清单见 [tasks.md](tasks.md)。
