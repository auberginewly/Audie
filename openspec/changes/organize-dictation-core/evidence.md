# 集成记录（2026-11-11）

## 实施范围

基线 main 5c38a85，工作分支 codex/reorganize-modules；用户已明确“全部完成”，本轮执行所有剩余代码整理。模板管理/循环键不回引；保留三份固定提示词和已合入的恢复修复。目录导航见 [docs/ARCHITECTURE.md](../../../docs/ARCHITECTURE.md)。关联前端与能力模块各有实施证据。

lib.rs 从 1667 行缩为装配入口，commands.rs 1014 行拆为功能命令与独立 settings；所有手写 Rust/TS/TSX/MJS 业务及测试源码均不超过 400 物理行。所有 Rust 内嵌测试已分离到所属模块，无生产文件内嵌测试断言。详细行数见 [清单](evidence/source-lines.tsv)。

## 自动验证

| 检查 | 结果 |
| --- | --- |
| cargo test --manifest-path src-tauri/Cargo.toml | lib 200 passed / 0 failed / 1 ignored；main/doc 0；原有会操作用户 Keychain 的测试继续忽略 |
| cargo fmt --manifest-path src-tauri/Cargo.toml -- --check | 通过 |
| cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings | 通过；比原必需检查额外覆盖测试目标。原历史 fixture helper 超参数阈值增加局部 allow 注释，正文不变 |
| pnpm test | 原 26 项全部通过 |
| pnpm typecheck / lint / format:check / build | 全部通过；lint 0 errors，34 条既有模式 warning |
| pnpm tauri build --debug --no-bundle | 通过，生成 target/debug/audie |
| 迁移前后 Cargo --lib -- --list | 均 201 项，忽略模块路径后的身份多重集完全一致，无丢失/新增 ignored |
| 全平台 Rust 源码测试审计 | 含 cfg(Windows) 的 209 个函数，正文和 test/cfg/ignore 属性全部一致（仅设置所属路径替换） |
| command 注册审计 | 40/40 invoke 名保留，参数/返回及 debug 门控不变 |
| 前端合同/文案 | 26 个测试身份、三语言各 256 个有效 key/value 共 768 项保持 |
| 浏览器 production preview + 模拟 Tauri IPC | 18 项断言通过，无运行异常；设置/历史/向导、三份提示词、胶囊状态和 undo/retry 派发。详见前端 change 的驱动、截图与 JSON |

编译器仍报告既有依赖 block 0.1.6 的 future-incompatibility 提示，没有为本次重构更新依赖。

## 语义审计与审查

独立只读审查分“方案符合”及“代码质量”两阶段，均无剩余可操作问题。先发现的 8 处旧内嵌测试已全部分离。

- 全部 Rust 基线 661 个函数定义/current 680 个。原函数正文差异仅在 run 装配、回调转交、Platform 窗口委托、Settings 默认 prompt 的 include_str 相对路径及 update_settings 热键函数所属路径；其余按字符串/注释保真词法比对。
- 能力模块原 289 个函数、129 个测试与属性、46 个类型正文保持；仅必要内部字段作用域变化。
- 原恢复流程锁范围、generation、模式/提示词/引用快照、旧结果隔离与转写复用保持。ActivePromptSlot 保留独立类型。主线程窗口隐藏实际执行时重查 generation + Idle；平台不读取业务 take。
- 所有设置消费者直接使用 settings，最终代码没有 commands 的兼容 loader 转发。宏注册使用实际命令子模块路径。
- 前端 AST 审计除 App 向导 hook 抽取和 Capsule command helper 所属调整外函数正文保持；没有把 Rust 业务迁入前端或假设跨 WebView 共享 Zustand。

## 未验证的系统边界

以下项目未自动勾选，不归档为整体验收完成；代码整理已实现，不再停留在规划。

| 项目 | 状态/原因 |
| --- | --- |
| 真实麦克风、ASR/LLM 联网、设备预览交接 | 未进行本轮真实录音/联网演示；单测/模拟 IPC 不证明硬件和 API 行为 |
| 原始 polish/rewrite/compose 快捷键、取消/恢复、跨 App 注入 | 自动恢复回归通过，真实应用及焦点行为仍待人工验收 |
| NSPanel ✓/✕不抢焦点、多屏/Space、连续录音 | 原生构建和代码审查通过，尚未做系统交互验收 |
| Keychain Reveal、真实配置迁移/重启、用户历史删改 | 规则/SQL/单测保持；未操作作者真实 secrets/历史以充当测试 |
| Windows 编译/测试/系统行为、Windows CI | 已有 CI 工作流保留；本轮仅本地实现并合并，未推送/触发 Windows CI，无 Windows 环境 |
| 搬迁前手动基线 | 未补作历史证据；总计划 1.3 保持开放 |

全量结果保留在各关联 change，待上述真实矩阵验证后再归档。

## 提交与独立检查点

- 00602d5：Rust 设置/入口/听写/平台职责。为让中间提交可用，暂存快照保留旧能力模块需要的 settings reexport 和原 provider 注册路径；对该快照独立 cargo test 200 passed / 1 ignored、clippy -D warnings 通过。
- 9aa0773：能力模块与就近测试，直接消费 settings；删掉临时转发并注册 provider 实际子模块。
- 5813965：前端功能目录、测试发现和证据；没有修改依赖及 lockfile。
- 最终文档提交更新总计划、STATUS 和代码导航；完整树再次核验后本地 fast-forward main，不推送。

React Doctor 完整同范围扫描基线 50/100 → 当前 52/100，均 38 条诊断；没有分数回退。仅迁移已有模式，未把诊断改善作为本次架构整理之外的功能修改。

## 工程后续（2026-11-11）

本记录上方是搬迁时的历史结果。后续已完成 pipeline 中文注释/显式导入与诊断隐私收口；Rust 222 passed / 1 ignored、前端 47 passed，完整生产代码 Windows/macOS CI 均通过。详见 [新集成记录](../harden-engineering-checks/integration-evidence.md)。真实麦克风、取消/恢复、空结果已部分观察，但有效粘贴和完整系统矩阵仍待确认，详见 [原生记录](../harden-engineering-checks/native-evidence.md)，不以 Success 事件替代目标字段结果。
