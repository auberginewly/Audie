# Tasks

## 1. 基线与测试搬迁

- [x] 1.1 保存当前生产源码和 Cargo 用例清单，确认基线为 main 35a7dbb 且既有恢复测试独立。
- [x] 1.2 将 lib/commands 内联测试迁至独立入口及职责分组，验证所有函数/属性/断言及生产正文与基线一致，新文件均少于 400 行。

## 2. 验证与进度

- [x] 2.1 完整 Rust 测试、fmt/clippy、前端测试/typecheck/lint/build 和 native build 通过；逐个 Cargo 测试身份与基线一致。
- [ ] 2.2 更新本片证据、STATUS 和父计划中实际完成的自动任务，独立提交；尚未完成的真机验收与后续业务整理保持未完成。

## Workflow follow-up

- 本片只完成测试分离，下一片为 settings/commands 职责整理，按单独 change 执行。
