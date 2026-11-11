# Design

## Context

以已合并恢复修复的 main 35a7dbb 为基线。lib.rs 2054 行，其中内联测试约 390 行；commands.rs 1595 行，其中内联测试约 580 行。另有 tests/recovery/ 的 10 项测试已独立，不重做。

作者已明确要求实施整体整理，本片是已确认 S0/S1 的具体文件落点，不是新的架构选择。

## Goals / Non-Goals

仅分离测试，业务字节保持一致，测试断言与属性保持一致；每个新测试文件少于 400 行。不改公开接口、前端、依赖或配置，不在本片拆业务职责。

## Decisions

- lib.rs 使用 cfg(test) + path="tests/lib_tests.rs" 的 tests 模块；commands.rs 指向 tests/commands_tests.rs，仍是内部单元测试。
- 两个入口 use super::*，通过显式 path 引用测试组，组内再 use super::*，保留原父模块私有项访问，不增加 pub。
- lib 组为 asr_config、enhance_config、take_and_stream；commands 组为 settings、providers、hotkeys、secrets。Windows cfg 与所有测试属性原样保留。
- 对比迁移前后生产片段的完整文本、每个函数及属性的去缩进文本、全部测试名称，确认没有遗漏或额外改动；Cargo 清单允许模块前缀变化，测试身份不变。
- 新文件均按职责取名，不把所有测试搬到外部集成测试目录；后续业务模块搬迁时对应测试随行。

## Risks / Trade-offs

- [子模块相对路径解析] → 每个模块显式 path，完整 cargo test 与 native build 验证。
- [只看总数漏掉替换用例] → 对照逐个用例身份及正文，包括本机不会执行的 Windows cfg 用例。
- [原生产文件仍超过 400 行] → 保留到后续职责拆分，不把大范围重构混入测试搬迁；进度明确只完成测试分离。

## Migration Plan

先保存源码与用例清单，再机械提取测试，运行 test/fmt/clippy、前端测试/构建及原生构建，更新 STATUS 和父计划。真实语音与跨 App 的未验收项沿用恢复切片，不将文件迁移验证冒充新的真机验收。
