# Design

## Context

2026-11-11 范围修订：取消多模板管理、候选列表和循环快捷键，保留原三份提示词编辑；恢复修复先作为独立切片落地，之后才继续本架构重构。

动机见 [proposal.md](proposal.md)。本设计记录作者在讨论中选择的方向：前端按功能收拢，Rust 保留已有 Manager + Command/Event，业务文件目标 400 行，测试独立且就近，macOS 优先。目录是迁移目标，不代表一轮执行或新增业务能力。

### 当前证据与基线

- src-tauri/src/lib.rs 约 2486 行，其中约 700 行测试；包含 setup、快捷键、录音上下文、处理、恢复、配置映射、事件及 NSPanel。pipeline.rs 仅有占位注释，尚未在 lib.rs 声明为模块。
- commands.rs 约 2117 行，混有 Settings/SettingsPatch、默认值、迁移与校验、持久化、快捷键应用和多个功能的 command；audio/history manager 与 provider_test 通过 commands 读取设置。
- commands::apply_hotkeys_if_changed 调用根模块的 build_hotkey_callback / HotkeyRole，形成入口与设置之间的反向依赖。
- 本次上下文分布在 LastTakeSlot、TakeGen、ActiveModeSlot、ActivePromptSlot、RewriteSourceSlot、ActiveStreamingSession。取消、恢复、缓存交接与清理存在同锁及执行编号约束；不是随意搬字段即可合并。
- macOS hide_overlay 在排队的主线程操作真正执行前，再检查执行编号及 Idle；不能只在调用时检查。
- 前端多数文件已较小；主要问题是同一功能横跨 components、hooks、types、store。useSettings 同时加载设置、供应商列表和麦克风；当前 main 的 TextSection 只编辑 enhance_prompt/rewrite_prompt/compose_prompt，不含模板管理。
- package.json 使用 Node 内置测试运行器，当前仅发现 tests/*.test.mjs；有 home-chart、settings-schema、onboarding-permissions 等定向脚本。ESLint 只配置 src/**/*.{ts,tsx}，Prettier 当前格式化范围亦不覆盖 mjs。
- tests/sidebar-version 与 release-version 含脚本/源码路径依赖，迁移要逐项修正，不能仅移动测试文件。
- 工作区已有原案所基于的模板与恢复改动；2026-11-11 作者已取消多模板管理与循环键，当前基线改为 main 加独立恢复修复；本设计不将这些内容视为本重构的新实现。历史测试数字不是当前测试结果。

### 参考及取舍

- [Handy AGENTS](https://github.com/cjpais/Handy/blob/main/AGENTS.md) 将 managers、commands、settings、shortcut、overlay 分开；本地参考版本 bc6a41e418dda66a1f8d0b123e6a83880a66b6a1 还具有 transcription_coordinator。采用职责边界，不复制本地模型管理或供应商无关的新层。
- [Voquill AGENTS](https://github.com/voquill/voquill/blob/main/AGENTS.md) 采用 TypeScript 业务与 Zustand 权威状态；与 Audie 的 Rust 热路径不同，不迁移为该模式。
- [Tambourine 架构](https://github.com/kstonekuan/tambourine-voice/blob/main/docs/architecture-diagram.md) 包含 Python/Pipecat 服务；不适用于本项目客户端直连边界。

## Goals / Non-Goals

**Goals:**

- 作者能按功能定位实现、类型与测试；入口负责组装，规则具有明确归属。
- 以小 diff 渐进迁移，每片可编译、可演示、可回退；不制造旧新两套实现长期并存。
- 新建及迁移完成的业务文件目标不超过 400 个物理行；测试文件单独计算，过长同样按场景细分。未轮到的旧文件不因全仓阈值阻塞前一片。
- 既有测试继续发现和运行，保持命令/事件/配置/历史与 UI 行为兼容。

**Non-Goals:**

- 不实现 agent、MCP/工具调用、语音命令或通用任务引擎；不改 Non-Goals。
- 不新增 Manager、事件总线、依赖注入框架、repository/service 层、crate、npm/cargo 依赖或测试框架。
- 不改 ASR/LLM 协议、同步/异步运行方式、网络取消、超时、连接复用、性能预算或提示词质量；这些另开行为变更。
- 不改变锁顺序、取消与注入边界、六秒终态窗口、提示词快照和重试读当前连接配置的语义。
- 不重画 UI、不修改视觉 token、不更换 Zustand/Zod、不自动生成前后端合同。
- 不要求 Windows 真机验收作为 macOS 切片的前置，但必须保持已有 Windows CI。

## Decisions

### 1. 功能优先组织，已有能力分层保持

同一功能、因同一需求变化的代码靠近；通用 UI 与平台细节保留各自边界。拒绝仅按 components/hooks/types 分类造成分散，也不引入完整 FSD/DDD 分层。文件接近 400 行先检查职责；例外在当片说明原因并交作者确认，不能删注释、压行或增加空转发来达标。

前后端使用对应业务词汇，但不强行镜像目录。前端发 command、校验 Rust event 并镜像；Rust 管听写决策。主窗口与 overlay 各自使用自己的 WebView store，跨窗口仍靠现有 Rust 事件同步，不能假设模块收拢后 store 就跨窗口共享。

### 2. 前端目标目录与归属

```text
src/
  main.tsx
  overlay.tsx
  App.tsx
  features/
    dictation/       # 胶囊、波形、录音事件、状态镜像和相关 hook
    history/         # HistoryScreen、查询 hook 与历史 schema
    home/            # HomeScreen、统计 hook、图表与日期纯函数
    providers/       # 模型配置、预设、列表/测试/本地发现及 schema
    text/            # TextSection 与原三份提示词编辑，不含模板库
    hotkeys/         # HotkeyRecorder、Windows 前端录制辅助
    audio/           # 输入设备、麦克风预览、设备列表/schema
    permissions/     # 权限 hook、PermissionRow、状态推导
    settings/        # 设置弹窗、通用/关于、完整设置 schema 与 useSettings
    onboarding/      # SetupWizard、步骤、进度推导
  components/
    ui/              # 不感知业务的 Button、Dialog、Input 等
    shell/           # 应用布局、导航及其测试
  lib/               # open、runtimePlatform 等确实跨功能的平台辅助
  i18n/              # 入口、语言元数据、按语言拆分的文案
  styles/            # 保持现有 token
```

功能目录先放实际需要的文件，不为每个功能创建空 components/hooks/services 子目录；测试存在时放 __tests__。大组件按有名称的界面职责拆开，例如 Capsule 与 Waveform、HomeScreen 与 UsageChart，不逐个 JSX 片段拆文件。

| 现有文件/组 | 目标归属 | 注意事项 |
| --- | --- | --- |
| Overlay/Capsule、useRecordingFlow、useAudioLevels、store/recording、types/events 的录音事件 | dictation | 权限/麦克风预览专属事件归各自功能；保留 event 名和 Zod 输入校验 |
| HistoryScreen、useHistory、types/history 的记录模型 | history | Home 共用的统计数据 schema 保留在 history，home 单向引用 |
| HomeScreen、homeChart、useDailyUsage、useUsageStats | home | 读取 history 导出的统计合同，history 不引用 home |
| ModelSection、ModelConfigDialog、LocalLlmCard、model-config、models、localModelRecommendations、useConfiguredModels | providers | 保留模型缓存/测试配置语义，不改测试按钮请求 |
| TextSection | text | 保留原三种模式各一份提示词的编辑，不增加管理器、候选列表或循环 |
| HotkeyRecorder、windowsHotkeyCapture | hotkeys | 设置与 onboarding 都消费同一能力 |
| ui/DevicePicker、useMicMonitor、设备列表读取 | audio | DevicePicker 即使外观通用，当前作为输入设备入口集中维护；其底层 Select 留 ui |
| PermissionRow、usePermissions、useInputMonitoring、permissionState | permissions | 请求后轮询、打开设置和重启引导不变 |
| SettingsDialog、GeneralSection、AboutSection、SettingSection、useSettings、完整 SettingsSchema | settings | GeneralSection 只组合功能，不重新实现它们 |
| SetupWizard、setup-wizard、App 的向导进度/首启协调 | onboarding | App 保留开关及导航，进度规则放 onboarding，hook 实例由 App 组合并共享，避免重复轮询 |
| i18n.tsx | i18n/index.tsx、languages.ts、locales/<language>.ts | 仅移位，key/翻译/默认语言不变；语言文件若超 400 行再按原有功能 key 前缀细分 |

依赖约束：

- App/settings/onboarding 组合 providers/text/hotkeys/audio/permissions；被组合功能不反向 import SettingsDialog、SetupWizard 或 App。
- useSettings 是现有 Settings 的读取、patch 和事件同步入口，移至 features/settings；供应商/设备列表的读取函数归各自功能。首轮允许 useSettings 保留同批 Promise.all 的组合与返回外观，不强制拆成多个独立 hook，避免改变既有失败与加载语义。功能可以 type-only 引用完整 Settings 合同，不能因此依赖设置页面或自行复制一份 schema。
- 原三份提示词字段继续归完整 SettingsSchema，不新增 PromptMode/PromptTemplateSchema。Provider 元数据、连接测试 schema 归 providers；设备 schema 归 audio。抽纯合同后先更新 type-only 消费者，避免形成运行时循环。
- features 允许上述有目的的单向依赖，不要求全仓任何功能都互不引用，也不建立万能 shared/types 或统一 IPC 封装。
- 不引入模板切换 toast、settings-updated 模板事件或循环快捷键；文本编辑仍经原 update_settings 保存。
- useSettings 对列表读取的职责收缩只允许保持现有加载时机、失败反馈及数据内容；按需创建职责明确的读取文件，不为每个 invoke 包一层。独立加载/错误 UI/请求竞态修复另开行为变更。

### 3. Rust 目标目录与归属

```text
src-tauri/src/
  main.rs
  lib.rs
  commands/
    mod.rs
    dictation.rs
    settings.rs
    history.rs
    providers.rs
    audio.rs
    permissions.rs
    secrets.rs
  pipeline/
    mod.rs
    recording.rs
    processing.rs
    recovery.rs
    context.rs
    config.rs
    events.rs
    tests/
  settings/
    mod.rs
    types.rs
    defaults.rs
    persistence.rs
    migration.rs
    validation.rs
    tests/
  hotkeys/
    mod.rs
    registration.rs
    capture.rs
    tests/
  managers/
    audio/              # mod、capture、monitor、buffer 与 tests
    history/            # mod、storage、queries、stats 与 tests
    transcription.rs
    enhance.rs
    inject.rs
  asr/                  # 现有 adapter 保留；长文件分离测试后再按协议职责拆
  llm/
    mod.rs
    openai_compatible.rs
    sanitize.rs
    tests/
  provider_test/
    mod.rs
    connection.rs
    models.rs
    tests/
  platform/
    mod.rs
    macos.rs
    macos/              # 增加 overlay.rs，保留其他现有实现
    windows.rs
    windows/
  state.rs
  error.rs
```

Rust 不能同时保留同名 foo.rs 与 foo/mod.rs 作为两套实现。迁移目录时原模块被替换，调用路径保持或一次更新；小文件不为目录美观强制拆开。

| 原职责 | 唯一目标归属 | 对外边界 |
| --- | --- | --- |
| run/setup、插件、manage、generate_handler | lib.rs | 只创建和连接现有对象 |
| handle_hotkey/start_recording/finish_recording/recorded_take | pipeline/recording | 快捷键和胶囊确认调用相同流程 |
| spawn_transcription/resolve_transcript/finish_pipeline_tail、正常结果历史写入 | pipeline/processing | 调既有 Manager，不重建另一套 provider |
| cancel/resume/undo/retry/insert_raw、enter_error、终态清理与取消历史协调 | pipeline/recovery | 同一缓存、执行编号与时序约束 |
| LastTake、DictationMode、HotkeyRole、上下文类型与槽位 | pipeline/context | 先保持数据布局和锁；仅必要符号 pub(crate) |
| ASR/LLM 配置 mapper、语言及提示词快照选择 | pipeline/config | 读取 settings，按需从 Platform 读 key，不持久化 key |
| FinalTranscript/ErrorPayload/EnhanceProgress 等 | pipeline/events | 保持序列化与事件名称；不创建 EventBus |
| 快捷键注册/恢复、begin/end_trigger_capture、回调构造 | hotkeys | 回调调用 pipeline；pipeline 不反向调用 hotkeys |
| Settings/SettingsPatch、提示词字段、load/persist/repair | settings | Manager/provider_test 从此读取，不经 commands |
| update_settings 的校验→快捷键/Dock应用→持久化等协调 | commands/settings | 沿用现有成功顺序及失败回滚；不让 settings 核心依赖 hotkeys/commands |
| Tauri command wrapper、open_main_window、history 重润色入口 | commands 对应文件 | 参数、返回及 invoke 名不变；history 显式重润色与 take 重试分开 |
| NSPanel、主线程窗口调用、多屏跟随 | platform/macos/overlay | 沿用 Platform 扩展最小初始化/显示/隐藏能力，Windows保留现有显示实现 |
| 日常音频采集/监听/缓冲、SQL读写/统计 | 原 AudioManager/HistoryManager 内部子模块 | 保留 Manager 入口和调用者，不新增音频/数据库层 |

状态机仍在 state.rs，决定合法状态迁移并发出现有 state-change；pipeline 决定何时申请迁移，平台实现不决定业务状态。原三份提示词的字段校验仍归 settings，不增加候选循环或模板通知。

### 4. 窗口隐藏与执行有效性不能因拆分失效

Platform 接管系统实现，但业务执行编号不能泄漏成每个平台各自的规则。采用现有 Tauri 主线程调度能力：pipeline 的隐藏请求在主线程闭包真正执行时复核 TakeGen 与 Idle，通过后调用平台隐藏方法；平台方法执行窗口操作，不读取 LastTake/TakeGen。显示亦保留原主线程要求。初始化/跟屏线程属于平台。

这只是职责迁移，不更换同步策略：保留 LastTake 锁与状态切换/清理的既有原子区段，网络操作在锁外，旧任务不重建已释放缓存；检查与系统粘贴之间的现有竞态边界不夸大为“绝对可撤销”。已有进入平台粘贴的调用仍不承诺撤回。

### 5. 测试独立文件，迁移不降低发现与覆盖

- 第一片仅将 lib.rs/commands.rs 测试移至 src/tests/ 下各自文件，通过 #[cfg(test)] 与显式 #[path] 保留为原模块的子测试模块；这是过渡落点，不把私有函数为测试改 pub。
- 业务迁移时测试随功能进入其 tests.rs 或 tests/mod.rs + 场景文件。纯搬迁比较测试用例身份（忽略模块路径前缀），不是只比较总数；允许有意新增的回归用例，但不删除失败用例凑通过。
- 前端保留 .test.mjs 与 Node test runner，功能测试落在功能/__tests__；shell 测试归 shell/__tests__；release-version 测试留根 tests/，它验证仓库脚本，不属于前端页面。
- package.json 的 pnpm test 同时发现根 tests 与 src 内 __tests__，跨 macOS/Windows 使用 Node 内置文件遍历/启动测试入口；保留测试失败 exit code、显式打印发现数量和零发现失败，不依赖 shell 的 ** 展开。只有确有需要才加一个很小的 scripts/run-tests.mjs，不引入框架。
- 所有定向 test:* 脚本和 CI 随路径同片更新。Node 测试直接 import 的 TS 纯模块仍可运行，不在该片新增 JSX/DOM 测试工具。
- 移入 src 的 mjs 测试使用 Node 作用域的 ESLint 配置与 Prettier 范围；业务 TS 保留严格类型检查，Node 测试不靠 any 或全局关闭规则适配。迁移时确认 tsconfig include、Vite 生产入口不导入测试，测试不进入运行时依赖图。
- 不新增“断言文件必须叫什么”的脆弱目录测试；保留已有源码检查测试并修正路径，其行为级替换另议。

## Risks / Trade-offs

- [已有工作区修改被重构覆盖] → 第一片前记录 dirty/untracked 文件与当前差异，建立包含现有行为的可恢复基线；基线提交/备份与重构提交区分，不 reset/clean、丢弃或擅自归档进行中切片。
- [恢复修复尚待真机验收] → 先演示相关 macOS 基线，原未验收状态继续保留；基线已失败的场景不靠重构修复，独立报告后缩小范围。
- [文件拆散却更难读] → 按业务职责拆，不为每个函数建文件；内部路径用有限 pub(super)/pub(crate)，不把所有符号全仓导出。
- [命令搬迁后 Tauri 宏注册遗漏] → 同片更新 generate_handler 的完整路径及宏所需可见性，实际 invoke 冒烟验证，不只依靠 cargo check。
- [文件名写法在 macOS 能过、Windows 失败] → 更新所有 import 大小写、使用 Git 可追踪的重命名，保留已有 Windows build/test/clippy CI；CI 通过不等同系统粘贴真机可用。
- [设置拆分破坏失败回滚或隐私] → 保留原 update_settings 的副作用顺序、快捷键回滚、presence-only 与显式 Reveal；不改序列化字段，不读取真实 key 作文档验证。
- [异步旧请求/定时器盖掉新执行] → 把原有归属检查与锁作为不可改变的搬迁约束，自动回归配合真实取消/重试/连续录音演示。
- [测试搬迁后没有运行但命令仍绿] → 记录迁移前后用例清单与数量，默认入口覆盖迁入与未迁入测试，零发现失败。
- [全仓一次重构无法回退] → 下面每个切片单独完成检查、演示、提交和 STATUS 记录；不自动从一片进入下一片。

## Migration Plan

### 执行方式

本 change 是整体组织调整的总体设计与验收清单，不是一个巨型 commit，也不直接全量 apply。每个实际实施片在 openspec/changes/<slice>/ 下建立独立关联 change，引用本设计并写明本片实际文件/任务及验收；完成该片后在这里回填进度，不复制整套设计。这样遵守项目“一片一方案、一片一提交”的规则。

后续普通“开始”默认先为 S0+S1 建立 separate-rust-tests 切片规划，审阅后由其 apply 实施；不会自动启动后续切片。本次请求只生成总体规划，不创建全部子 change。结构遵守本方案，若需新增依赖/架构层再讨论。

| 切片 | 依赖 | 本片范围与演示 |
| --- | --- | --- |
| S0 基线 | 无 | 保护工作区、列当前自动检查结果，演示一次听写及提示词快照/恢复基线；记录未验证项 |
| S1 测试分离 | S0 | lib/commands 测试独立文件；测试用例不丢，核心听写行为不变 |
| S2 设置规则 | S1 | Settings/默认值/迁移/提示词字段/持久化移 settings，修正 managers/provider_test 消费者；设置保存、重启、原三份提示词兼容 |
| S3 command 入口 | S2 | 按功能迁移命令，保留暂未迁移业务的薄调用；设置、设备、权限、历史、模型测试与 Reveal 各入口可达 |
| S4 平台窗口 | S3 | 最小 Platform 窗口能力及 macOS overlay 模块；按键和按钮不抢焦点，多屏/Space、连续录音隐藏正确 |
| S5a 听写上下文与配置 | S4 | 建立 pipeline，迁移上下文、配置 mapper、事件及测试；保留现有槽位锁与行为，原入口可暂作薄转发 |
| S5b 录音与输入 | S5a | 开始/停止交接、快捷键注册/捕获移入 pipeline/recording 与 hotkeys；主/写作键触发、录键后恢复正常 |
| S5c 处理与恢复 | S5b | 处理/取消/重试/清理移入 pipeline，移除 lib 中转发；模拟回归及真实录音/跨 App 粘贴演示 |
| S6a 模型与注入 | S5c | llm/provider_test 按请求、输出清理、模型查询拆分及测试；同供应商、请求与输出兼容 |
| S6b 音频模块 | S6a | AudioManager 内部采集/监听/缓冲拆分及测试；设备预览、录音、队列溢出行为兼容 |
| S6c 历史模块 | S6b | HistoryManager 内部存储/查询/统计拆分及测试；原数据库无需迁移，历史及首页统计相同 |
| S7a 前端合同与设置 | S6c | 收拢纯 schema/Settings hook，归属 providers/audio 列表读取函数并更新消费者；保留原组合加载语义，原三份提示词、配置及测试发现正常 |
| S7b 模型与文本设置 | S7a | providers/text 组件收拢并更新 Settings/onboarding 消费；配置、连接测试和单份提示词编辑不变 |
| S7c 输入、权限与向导 | S7b | hotkeys/audio/permissions/onboarding 收拢，Settings只组合；录键、预览、授权状态与向导正常 |
| S7d 听写界面 | S7c | dictation store/events/hooks/Capsule收拢，按需拆Waveform；主窗口/overlay状态、按钮和动效不变 |
| S7e 历史与首页 | S7d | history/home收拢，统计合同单向引用；历史操作与7/30/60天图表一致 |
| S7f 应用壳与文案 | S7e | App保留组合、向导规则就近，i18n按语言/功能拆；导航、首启与三语言一致 |
| S8 整体验收 | 所有已选实施片完成 | 核对目录/依赖/行数/测试、macOS闭环与Windows CI；未完成的片仍保持未勾选，不提前归档 |

S6 的三个能力拆分不能因为仅剩长度问题就重写算法；分离测试后已经小且职责单一的文件可原样保留，并在任务结果说明。每片的测试与文档随本片完成，不留到 S8 才处理。

### 检查与证据

- Rust 片：记录 cargo test --manifest-path src-tauri/Cargo.toml --lib -- --list 清单与实际测试结果；cargo fmt --manifest-path src-tauri/Cargo.toml -- --check、cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings；相关片包含 pnpm tauri build --debug --no-bundle，验证 native 入口与命令注册。
- 前端片：pnpm test、pnpm typecheck、pnpm lint、pnpm format:check、pnpm build；每片在真实 Tauri 或可验证页面执行对应 UI 操作。默认测试入口与原定向脚本都能运行，避免只执行旧目录。
- macOS 手动矩阵：普通听写；有选区改写；写作；三模式提示词编辑与恢复快照；录音中取消/撤销；处理中取消；错误重试/原文插入；旧六秒定时器跨重试；快速开始下一次录音；✓/✕不抢焦点；跨 App 注入；多屏及 Space；设置和历史重启保留。网络错误用隔离模拟或可恢复配置，不破坏用户正常 key。
- 所有命令输出只作为当片证据，不继承旧文档的“通过”；Windows CI 不可访问则记未查，不自行声称通过。
- 每片 STATUS 记录实际检查、演示、已知基线失败、未覆盖项和提交；尚未被选中的后续片不执行。

### 回退

只回退本片提交或恢复本片前记录的文件；遇到作者/其他会话修改时使用定向恢复，不使用全仓 reset --hard/clean。没有设置或数据库格式迁移，回退不要求恢复用户数据。不要通过保留第二套配置加载器或第二套 pipeline 实现维持过渡；允许当片可追踪的薄转发，在对应后续片移除。
