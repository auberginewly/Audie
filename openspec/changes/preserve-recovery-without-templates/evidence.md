# 实施记录 · 2026-11-11

## 范围与基线

- 隔离 worktree：`/Users/aubergine/.codex/worktrees/recovery-without-templates/Audie`。
- 分支：`codex/recovery-without-templates`，基于 main `3ff9d20`。
- 只迁入 `6da365d` 的恢复相关行为并适配原三份提示词；原 feat/sound-feedback 与原 main 工作区未修改。
- 基线 Rust：190 passed / 1 ignored。没有模板集合、模板管理弹窗、循环快捷键或相关新增配置。

## 实际改动

- 保留录音模式、原提示词正文、改写引用与执行编号；正常与恢复共用处理路径，已转写内容直接复用。
- 原文插入绕过 LLM，改写插回原选中文字；历史使用原模式和引用。
- 取消后迟到的结果/错误不发起注入或发布旧终态；旧执行不更新新缓存，旧定时器不清新缓存或隐藏新胶囊。
- 提示词快照使用独立 Rust 包装类型，避免 Tauri 按类型存 state 时与改写引用槽位冲突。
- 新增 10 个恢复回归测试，独立文件为 mod.rs 21 行、processing.rs 221 行、lifecycle.rs 61 行、snapshots.rs 46 行，均低于 400 行。
- commands.rs、前端、Platform、快捷键槽位、Cargo/package 依赖与锁文件、默认提示词文件均与 main 一致；没有配置/数据库迁移。
- 整体架构方案已将多模板迁移改回原 TextSection 与三份提示词字段；后续架构任务仍未实施。

## 自动验证

| 检查 | 本轮结果 |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib` | 200 passed / 1 ignored，新增 10 项恢复测试全部通过 |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 通过 |
| `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | 通过；Cargo 另提示既有 block 0.1.6 未来兼容性问题 |
| `pnpm test` | 26 passed，0 failed |
| `pnpm typecheck` | 通过 |
| `pnpm lint` | 0 error / 34 条现有 warning |
| `pnpm format:check` | 通过 |
| `pnpm build` | 通过 |
| `pnpm tauri build --debug --no-bundle` | 通过，生成本 worktree 的 target/debug/audie |
| `openspec validate --all --strict --no-interactive` | 两项 change 均通过 |
| `git diff --check` | 通过 |

## 负对照与测试边界

先加入恢复测试后，main 因缺少快照字段与共用处理函数不能编译；这是接口缺口的证据，不冒充行为级失败。实现后另做两个行为级负对照：

1. 临时关闭 finish_pipeline_tail 的执行有效性检查，`recovery_cancel_during_llm_discards_success_and_failure` 按预期失败，错误为 cancelled LLM result must not inject。
2. 临时关闭 settle_take 的旧执行拒绝检查，`recovery_old_timer_cannot_clear_a_resumed_takes_cache` 按预期失败。

两次均以 finally 恢复正确源码，之后 10 个恢复测试再次全部通过；错误版本未提交。独立状态类型测试验证提示词与引用不共用类型身份及存储，不能据此声称真实 Wry 录音流程已验收。

完整本地日志在 `/tmp/audie-recovery-selected/`，包括 baseline-tests、recovery-before/after/restored、negative-cancel/timer、full-tests、clippy、frontend-tests、typecheck、lint、format、frontend-build 与 native-build。

## 待手动验收

真实麦克风、全局快捷键、三模式录音后的撤销/重试、改写原文插入、系统选区与跨 App 粘贴未在本轮自动操作；当前没有可用 Reticle 工具。已有进入系统粘贴调用的操作不承诺撤回。Windows 未实测。

这些项目仍保持未勾选；不归档本 change，也不推进后续架构切片。

## 提交

- `0bb847c`：记录架构组织方案与取消多模板的取舍。
- `fb4c2a2`：迁入恢复修复、独立回归测试与本片验证记录。
- 当前仅真实手动验收项未完成；不声称全部任务完成，不自动合并 main。
