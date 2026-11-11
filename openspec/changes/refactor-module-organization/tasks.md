# Tasks

2026-11-11 作者在 S1 后明确要求“全部完成”，本轮连续执行剩余代码整理，不再每片停下询问。独立实施记录见 [organize-dictation-core](../organize-dictation-core/evidence.md)、[organize-rust-capabilities](../organize-rust-capabilities/evidence.md)、[organize-frontend-features](../organize-frontend-features/evidence.md)。代码迁移完成；混合真实系统验收的任务保持未勾选，不能把单元测试/模拟 IPC 冒烟当作真机验收。1.3 的旧基线演示已无法补作搬迁前证据，当前手动矩阵列于集成记录。

Rust 检查组 R：cargo test --manifest-path src-tauri/Cargo.toml；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check；cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings。

前端检查组 F：pnpm test、pnpm typecheck、pnpm lint、pnpm format:check、pnpm build。搬迁测试时另核对用例清单，不能只看退出码。历史检查数字不作为本轮证据。

## 1. S0 — 保存并验证基线

- [x] 1.1 记录当前分支、HEAD、dirty/untracked 文件及可恢复基线，将既有恢复/文档修改与本重构区分；验证基线能还原且没有丢弃用户改动。
- [x] 1.2 记录 Rust --lib -- --list 与 Node 测试用例清单，运行 R/F 及 pnpm tauri build --debug --no-bundle；逐项记录真实通过、失败或环境限制，不自动修复无关失败。
- [ ] 1.3 在 macOS 演示普通听写、改写/写作、提示词编辑和取消/重试基线；将实际覆盖、既有失败和未测场景写入本 change 实施记录与 STATUS，确认后才进入搬迁。

## 2. S1 — 测试与业务分文件

- [x] 2.1 将 lib.rs 测试迁至 src/tests/lib_tests.rs，commands.rs 测试迁至 src/tests/commands_tests.rs，通过各自 cfg(test)/path 保留原子模块访问；验证非测试编译不包含这些文件、没有为测试扩大生产可见性。
- [x] 2.2 拆开超过 400 行的测试文件时按已有恢复/配置/提示词等场景分文件；运行 R，对照忽略模块前缀的用例清单，确认无丢失和意外新增 ignored。
- [ ] 2.3 演示一次快捷键听写与胶囊收尾；检查本片 diff 仅测试位置和模块声明，更新 STATUS 并独立提交，记录后续随业务迁移的测试落点。

## 3. S2 — 设置数据、规则及持久化

- [x] 3.1 将 commands.rs 的 Settings/SettingsPatch、默认值、load/write/migration、提示词字段与快捷键校验迁入 settings 对应文件；保留 serde/TOML 字段及旧配置修复规则，运行相关迁移、校验与持久化测试。
- [x] 3.2 将 lib、AudioManager、HistoryManager、provider_test 等设置消费者改为 settings，不依赖 command handler 取配置；执行 R，确认设置核心不反向调用 commands/hotkeys，未引入第二套 loader。
- [ ] 3.3 在隔离配置中验证旧配置迁移、提示词正文保留、冲突拒绝、保存及重启保持；复查 Keychain 边界未变，记录行数/结果并更新 STATUS、独立提交。

## 4. S3 — command 按功能组织

- [x] 4.1 将现有命令迁入 commands/{settings,dictation,history,providers,audio,permissions,secrets}.rs，更新 generate_handler 与必需可见性；尚在 lib 的流程只经薄调用接入，原 invoke 名、参数/返回与 debug 门控不变。
- [x] 4.2 保留 update_settings 的副作用顺序、快捷键失败回滚及 Dock 应用，history 重润色保持独立入口；运行 R/F 和 native build 验证宏注册及调用方编译。
- [ ] 4.3 在运行应用中分别调用设置、设备预览、权限查询、历史、provider 测试和显式 Reveal 入口；打开设置不读明文，记录冒烟结果与迁移后入口清单，更新 STATUS 并提交。

## 5. S4 — 平台窗口边界

- [x] 5.1 将 NSPanel 初始化、多屏定位/跟随及原生显示隐藏移入 platform/macos/overlay.rs，经现有 Platform 增加最小窗口能力；保留 Windows 原有窗口行为，运行 R/native build。
- [x] 5.2 保留实际主线程执行时的 TakeGen/Idle 再检查，将业务归属校验留调用侧；回归旧定时器/新录音不互相清理的用例，检查平台实现不读取 LastTake/TakeGen。
- [ ] 5.3 macOS 验证 ✓/✕不抢焦点、跨 App 注入、多屏/Space 和连续录音；记录有无多屏实测及未覆盖范围，更新 STATUS、行数与提交。

## 6. S5a — 听写上下文、事件与配置

- [x] 6.1 用 pipeline/mod.rs 替换未接入的 pipeline.rs 占位，迁移 LastTake/DictationMode/HotkeyRole/槽位至 context，事件至 events，ASR/LLM mapper 至 config；保持数据布局、序列化及同一份 managed state，运行相关配置/提示词快照测试。
- [x] 6.2 将原配置与上下文测试随模块迁移，调整必要 pub(super)/pub(crate)；执行 R/F，确认没有复制第二套状态、切换 async 或提前读取 key，比较用例清单。
- [ ] 6.3 演示三个模式、录音时提示词快照与修正连接后的重试；更新 STATUS/模块说明和行数，独立提交。

## 7. S5b — 录音入口与快捷键接线

- [x] 7.1 将 start/finish/recorded_take 及所需录音准备流程迁入 pipeline/recording，将 build_hotkey_callback、注册/恢复和触发键录制会话移入 hotkeys；commands/settings 调用 hotkeys，hotkeys 调用 pipeline，消除根模块回调依赖。
- [x] 7.2 保持 start 前台 App 快照、选区探测、权限/设备失败路径、流式最终帧与 LastTake 交接锁的先后；运行 R/native build，保留失败与交接相关回归。
- [ ] 7.3 验证主/写作快捷键、✓与二次按同源、设置录键暂停及恢复；更新 STATUS 并提交，确认本片未改变快捷键手势。

## 8. S5c — 处理、恢复与终态清理

- [x] 8.1 将转写解析、润色/改写/写作分派、注入及结果历史迁入 processing；取消、恢复、原文插入、错误和 settle 迁入 recovery；共用原上下文与执行校验，避免正常/恢复两套流程。
- [x] 8.2 将恢复/处理测试归入 pipeline/tests，执行 R/F/native build；覆盖转写复用、提示词/引用冻结、取消后迟到成功/错误、旧定时器与缓存释放，保留实际系统粘贴不可撤回边界。
- [ ] 8.3 真机演示三模式录音、处理中取消、撤销/错误重试、改写插入原文、旧定时器到期后新胶囊仍可操作及跨 App 粘贴；移除 lib 中已不需要的流程转发，记录结果、行数、STATUS 并提交。

## 9. S6a — 模型请求、输出清理及连接测试

- [x] 9.1 将 llm/mod.rs 的请求实现和 sanitizer 分别归 openai_compatible/sanitize，provider_test 按连接和模型查询拆分，测试独立；维持 trait、请求正文、鉴权、错误映射及模型缓存语义，运行对应测试与 R。
- [x] 9.2 检查 ASR 长文件，先独立测试，再仅对仍职责混合/超过目标的文件按现有 config/codec/client 模式细分；不改协议或复用策略，运行 adapter 测试并记录保留小文件的理由。
- [ ] 9.3 验证现有 provider 连接/模型列表界面与一次真实听写；检查没有新增依赖、网络/超时策略或 key 读取入口，更新 STATUS、行数并提交。

## 10. S6b — AudioManager 内部职责

- [x] 10.1 在原 AudioManager 下拆采集、预览、缓冲/流式转发与测试，保留公开入口、线程所有权、CPAL 资源释放和队列保险丝；运行采集辅助、buffer 与 overflow 回归和 R。
- [ ] 10.2 验证预览→录音麦克风交接、设备选择、正常结束与取消；确认无额外音频副本或落盘，更新 STATUS/行数并独立提交。

## 11. S6c — HistoryManager 内部职责

- [x] 11.1 在原 HistoryManager 下拆 storage/queries/stats 与测试，保持数据库路径/结构、保留策略和统计口径；运行历史存储/统计测试与 R，确认无需数据迁移。
- [ ] 11.2 用现有历史验证查询、删除/清空入口与首页统计，重启后结果不变；不破坏用户历史作测试，记录验证、行数、STATUS 并提交。

## 12. S7a — 前端纯合同、设置与测试发现

- [x] 12.1 将设置/provider/设备/history/事件 schema 分配到目标功能，收拢 useSettings 及其同批加载依赖，逐个更新消费者；只保留有目的的单向运行时依赖，执行 F 检查 Zod 合同与类型未漂移。
- [x] 12.2 将已有可归属的纯模块测试按功能移入 __tests__；调整 pnpm test 为跨平台同时发现根 tests 与模块测试，保留失败退出码并拒绝零发现，更新定向脚本及 Node mjs lint/format 范围，比较用例清单后运行所有默认/定向命令。
- [ ] 12.3 验证模型/设备/设置的原加载与保存体验、事件更新及重启保留，确认没有顺手改变 Promise.all 失败语义或新增错误 UI；记录目录迁移和测试结果，更新 STATUS 并提交。

## 13. S7b — 模型与文本设置收拢

- [x] 13.1 迁移 Settings 内模型配置相关组件、预设、动作和 hook 至 providers，更新 Settings/onboarding，保留相同组件实例和数据流；执行模型目录相关测试及 F。
- [x] 13.2 迁移原 TextSection 至 text，保留三种模式各一份正文的编辑，相关纯函数及测试就近；验证 enhance/rewrite/compose 字段保存与重启保留，不引入多模板或循环键。
- [ ] 13.3 实际设置及向导操作模型配置/连接测试、原三份提示词编辑，检查 UI/文本未变与业务文件行数；更新 STATUS、迁移映射并提交。

## 14. S7c — 输入设备、权限与配置向导

- [x] 14.1 将 HotkeyRecorder/Windows capture、DevicePicker/useMicMonitor、权限组件/hook/推导分别迁入 hotkeys/audio/permissions，修正相关事件/schema 与测试路径；执行 F，保持通用 Select/Button 在 ui。
- [x] 14.2 收拢 SetupWizard/steps/progress 至 onboarding，GeneralSection/SettingsDialog 组合现有功能；App 维持共享 hook 实例，运行权限/向导/录键测试，避免重复授权轮询或设备监听。
- [ ] 14.3 验证快捷键录制和恢复、设备预览、真实权限状态、关闭/重开向导及重启保留；无需重置系统 TCC 才能完成结构验收，记录未覆盖系统弹窗、STATUS 并提交。

## 15. S7d — 听写界面、事件及镜像状态

- [x] 15.1 将 Capsule/useRecordingFlow/useAudioLevels/store/相关事件归 dictation，按波形与终态展示等可命名职责拆组件，更新 App 与 overlay 入口；执行 F 并核对生产入口没有导入测试。
- [ ] 15.2 实际验证 RECORDING/PROCESSING/SUCCESS/ERROR/CANCEL、计时/音量及按钮；两个 WebView 各自接收事件，不假设 Zustand 跨窗口共享，保持现有视觉和动效。
- [x] 15.3 核对 TS 外部输入仍经 Zod，前端未接管听写决策，迁移测试并记录行数、UI 证据，更新 STATUS 并提交。

## 16. S7e — 历史与首页

- [x] 16.1 收拢 HistoryScreen/useHistory/schema 至 history，HomeScreen/chart/统计 hook 至 home；统计数据合同归 history，home 单向消费，迁移 home-chart 测试并执行 F。
- [ ] 16.2 实测历史查看、重新润色与无破坏性删除流程，核对首页 7/30/60 天请求、日期/曲线和空数据展示；记录结果、行数、STATUS 并提交。

## 17. S7f — 应用组合、公共组件与多语言

- [x] 17.1 精简 App 为导航/弹窗/功能组合，向导协调就近放 onboarding，公共布局留 shell；迁移 sidebar-version 测试并更新其源码路径，执行 F。
- [x] 17.2 将 i18n.tsx 按入口/语言元数据/各语言文案移动，超长语言文件按原有功能 key 前缀拆分；验证 key 集、原文、默认语言及全部消费者兼容，不新增国际化框架。
- [ ] 17.3 真机核对侧栏、关于/版本、首启和重开向导、中/繁/英界面与 overlay；检查没有万能 shared/utils 桶，记录最终映射、行数、STATUS 并提交。

## 18. S8 — 全部已实施范围的最终集成验收

- [x] 18.1 对照设计审查模块依赖、入口职责、400 行目标和测试归属；逐项说明保留文件/超长例外，清理已完成迁移的薄转发，不删除其他未完成切片的代码或文档。
- [x] 18.2 执行 R/F/native build，核对全量测试发现清单与 Windows CI；实际未运行或不可访问的检查明确记未验证，不将编译成功当作 Windows 系统行为验收。
- [ ] 18.3 完成 design 中 macOS 手动矩阵，核对命令/事件、设置格式、历史数据和三模式恢复未变；更新 STATUS 与架构/协作文档的真实模块位置，记录提交，所有未验收项保留未勾选。

## Workflow follow-up

- 代码范围按作者明确要求全部实施，分前端、Rust 入口/流程及能力实现提交，集成记录单独维护。
- 每个关联 change 仅记录实际验证结果；真实录音、联网 provider、焦点、多屏和 Windows 未测项保持开放，不归档为全验收。
- 本变更 skip_specs，不向 openspec/specs 同步虚构的产品能力。

## 工程后续验证（2026-11-11）

[工程收口记录](../harden-engineering-checks/integration-evidence.md) 已补 Rust 222 passed / 1 ignored、前端 47 passed、R/F/native build，以及完整生产代码 abb12dd 的 [Windows/macOS CI](https://github.com/auberginewly/Audie/actions/runs/42979405201) success，因此 18.2 已完成。原生只部分验证，见 [实际系统记录](../harden-engineering-checks/native-evidence.md)；真实粘贴、三模式/焦点/多屏等未证实项仍不勾选，历史基线 1.3 不补造。
