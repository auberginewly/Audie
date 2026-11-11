# Audie 工程规范

本规范用于新增和本次修改涉及的代码。沿用既定 Tauri 2 + Rust + React/TypeScript 架构，优先让维护者能读懂和修改；已有代码随对应切片整理，不为统一外观开展无关重写。

开发与检查入口见 [CONTRIBUTING.md](../CONTRIBUTING.md)，模块归属和允许的依赖见 [ARCHITECTURE.md](ARCHITECTURE.md)。需要改变架构、依赖或产品范围时，先取得维护者确认。

## 文件与命名

| 对象                      | 约定                                              | 例子                                      |
| ------------------------- | ------------------------------------------------- | ----------------------------------------- |
| Rust 文件/模块、函数/变量 | `snake_case`                                      | `recording.rs`、`finish_recording`        |
| Rust 类型/trait、Manager  | `PascalCase`，Manager 用 `XxxManager`             | `LastTake`、`AudioManager`                |
| Rust 常量                 | `SCREAMING_SNAKE_CASE`                            | `TERMINAL_HOLD_MS`                        |
| React 组件及其文件        | `PascalCase`                                      | `HistoryRow.tsx`                          |
| Hook 及其文件             | `useXxx`                                          | `useRecordingFlow.ts`                     |
| TS 函数/变量              | `camelCase`；类型用 `PascalCase`                  | `callOverlayCommand`、`Settings`          |
| 功能目录、纯逻辑文件      | 功能目录小写；纯逻辑文件沿用 `camelCase` 或职责名 | `dictation/`、`homeChart.ts`、`schema.ts` |
| Zod schema                | `XxxSchema`，优先从 schema 推导对应类型           | `SettingsSchema`                          |
| 前端纯测试                | 功能目录中的 `__tests__/*.test.mjs`               | `home/__tests__/home-chart.test.mjs`      |

- 手写 Rust/TS/TSX/MJS 源码、测试与工程脚本不超过 400 个物理行，包含空行与注释，末尾换行不另算空行。测试独立计算，过长同样按场景拆分；生成文件除外。`pnpm check:engineering` 检查 src、src-tauri、scripts、tests 及根目录同类文件，排除 node_modules、target、dist、dist-ssr 和 src-tauri/gen 构建/生成目录；零发现视为失败。
- 按经常一起修改的职责拆分，不按固定行号分割。必要的超长例外先解释取舍并由维护者确认。
- 测试、类型和纯逻辑靠近拥有它的功能；公共目录只接收确实跨功能使用、且不反向依赖业务的内容。
- 类型最小可见：Rust 优先私有，再按实际协作需要用 `pub(super)` / `pub(crate)`；不为测试公开实现细节。
- 新增生产代码优先显式 import/use，纯类型引用使用 `import type`。不要为一个消费者建立全仓聚合导出；已有有意设计的模块入口可以保留。
- 前端沿用现有相对导入，不为修改规范引入路径别名或重命名所有旧文件。跨功能依赖遵守架构文档，不机械禁止所有 feature 引用。

## 注释

新增或重写的说明以中文为主，代码标识符、协议字段和 API 名保留原文。已有准确的英文注释无需单独翻译；修改相关逻辑时优先修正过期描述。

| 位置                            | 需要解释什么                                                     | 形式                                                            |
| ------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------- |
| 模块入口                        | 本模块职责和关键边界，通常 1–3 句                                | Rust `//!`；TS `//`                                             |
| 核心类型、跨模块函数或复杂 hook | 业务含义、调用条件、重要副作用；有需要时补错误/线程/生命周期约定 | Rust `///`；TS `/** … */`                                       |
| 实现内部                        | 不直观的原因、时序、必须保持的约束，以及看似多余的代码为何不能删 | `//`                                                            |
| 系统调用、并发、unsafe          | 主线程要求、锁保护什么、对象何时释放，以及安全前提由谁保证       | 临近实现的说明；unsafe 函数用 `# Safety`，unsafe 块说明安全依据 |
| 测试                            | 从场景和断言仍不容易看出的回归风险                               | 就近的 `//`                                                     |

模块和接口可以说明“做什么”；实现注释重点回答“为什么”。不逐行翻译赋值、循环或简单 getter，不重复抄写 TypeScript 已声明的参数类型，也不要求所有私有函数都写文档。

例如：

```rust
/// 当前仍允许输出文字的听写任务编号。
/// 取消或重试时递增，防止旧任务晚返回后误粘贴。
type TakeGen = Arc<AtomicU64>;
```

- Rust 不熟悉的机制只有影响业务时才补充，例如 Tauri 按具体类型存 state，为什么提示词 slot 需要独立类型。通用语法教学放文档，不铺在每一行代码旁。
- 有实际错误或 panic 条件时说明条件和副作用；不为凑模板堆空的 `Errors` / `Panics` / `Examples`。
- TODO 写出缺口、影响及完成条件；有 issue 时附链接。不要只写“以后优化”。
- 不写“片 2 新增”“P1 将实现”、作者/日期流水账或被注释掉的旧实现；阶段记录归 OpenSpec，代码历史归 Git。
- 改逻辑时同步核对相邻注释；注释含路径/函数名时保证仍可定位。不为满足 400 行目标删必要注释。

## Rust 执行与状态

- 沿用现有 Manager、Platform/provider trait 和唯一一份听写上下文，不复制第二条正常/恢复流程。
- command 是 UI 入口，负责参数接入及必要的既有副作用协调；录音、处理、取消、恢复决策在 pipeline，操作系统细节在 Platform 后。
- 现有录音、ASR、LLM 工作线程保持职责。异步任务沿用 Tokio；阻塞推理、文件/网络工作放现有专用线程或合适的 `spawn_blocking`，不放进 UI/异步运行线程；不为统一风格改成另一种执行模型。
- 共享状态需明确锁保护范围，不随意拆开 generation 校验与缓存/注入交接的临界区，也不扩大到不必要的网络等待。
- 取消、重试与定时器使用同一任务归属校验；原生窗口操作排到主线程后，仍须在实际执行时核对所属任务和状态。
- 模式、提示词正文与引用使用录音时的快照；重试可读取当前连接配置以便修正 endpoint/key。不要恢复旧连接配置或重新探测选区覆盖原任务上下文。
- Rust 是听写状态的权威。两个 WebView 各自接收事件，不能假设共享同一个 Zustand 实例。

## Command、event 与持久化合同

- 沿用 command 的 `snake_case` 和 event 的 `kebab-case` 名称；文件搬迁不改 invoke 名、参数映射、payload 字段、序列化或 debug 门控。
- 新增或修改合同，在同一个 change 里写明名称、请求/返回或 payload、错误/取消语义；同时改 Rust 定义、TS schema、消费者及相关测试。沿用模块内合同，不先造一套注册框架。
- TS 外部输入，包括 invoke 返回、Rust event、配置和 API 响应，以 `unknown` 接入并用 Zod 校验后使用；不通过 `any`、双重断言或非空断言绕开验证。
- command 的错误用调用结果反馈；后台听写错误经原状态/event 通道处理。避免同一个失败在多层重复发 toast、重试或改状态。
- 配置由 `settings/` 唯一 loader 读取，持久化至 `settings.toml`；旧 JSON 仅用于迁移。历史由 HistoryManager 存 SQLite；密钥只在系统凭据存储。
- 新配置字段需一起考虑默认值、旧配置缺失、校验、patch 语义及重启保持。区分“未传”和“清空”，不默默改变既有字段含义。
- 打开设置仅检查密钥是否存在；显式 Reveal 或 provider 实际需要时才读明文。不要把凭据放进普通 state/event、导出文件或测试快照。

## 错误与日志

- 应用边界沿用 `AppError` / `AppResult`。库层需要自定义错误时沿用 `thiserror`；不为单个切片引入新的错误框架。
- 错误分类沿用 `Permission / Device / Network / Provider / Inject / Internal`；分类、可恢复性和 UI 行为一起评审，不把所有失败都降为 Internal。
- 热路径不使用 `unwrap()` / `expect()`。合法默认值、可选能力或降级可以显式处理；不能静默把实际失败变成空文本或成功。
- 低层补充排查所需上下文，在拥有处理职责的边界记录错误，避免同一次失败逐层重复打印。预期取消不按故障记录。
- 日志优先记录阶段、非敏感任务编号、耗时、长度及状态码；不记录 API key、Authorization、录音、完整听写/提示词/选区正文，或未脱敏请求/响应体。
- 不把带凭据/query 的 URL 或完整底层错误直接打印；输出前保留排查必需的安全字段。调试日志与提交到 issue 的材料遵守相同隐私边界。
- 用户提示说明发生了什么及可采取的动作。新增界面文案进入现有 i18n，三种语言的 key 保持一致；不在公共 UI 流程展示线程、锁或内部模块名。

## 前端与资源生命周期

- 组件负责渲染和交互接线，状态协调放所属 hook/store，纯计算独立便于测试。不要因组件变长就抽一层无业务含义的包装。
- event listener、定时器、设备预览与轮询由创建者负责停止/释放；处理订阅异步完成前已卸载的情况，避免重复订阅与更新已结束会话。
- 设置、权限、设备等共享 hook 实例在现有组合位置复用；不要因为文件移动创建多份轮询或更改原 `Promise.all` 的失败语义。
- 样式沿用 Tailwind v4 与现有主题 token；通用样式和控件在 ui/shell，单一功能的行为不塞入公共组件。

## 测试与验收

Rust 单测放所属模块 `tests.rs` / `tests/`，由 `#[cfg(test)] mod tests;` 接入；前端纯测试放功能 `__tests__/*.test.mjs`。发布及工程脚本测试留根 `tests/`。沿用现有 Cargo/Node runner，不为补规范替换测试框架。

| 改动                         | 至少检查什么                                               | 补充验收                                                                 |
| ---------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| 纯文档                       | Markdown 格式、相对链接、命令/路径与实现对照、diff 空白    | 不需要录音或全量业务测试                                                 |
| 纯逻辑、schema、配置规则     | 原有用例及相关回归、所属语言静态检查                       | 配置字段变化补缺省/patch/旧配置兼容场景                                  |
| 前端 UI/hook                 | 前端测试、typecheck、lint、format、build；实际操作相关界面 | 浏览器模拟 IPC 与真实原生分别记录；界面变化附图                          |
| Rust 入口/模块/command 整理  | Cargo test/fmt/clippy；前端检查与原生 build 验证跨语言接入 | 比对 invoke/event/字段；移动测试比对用例身份与 ignored，不只看退出码     |
| 听写处理或恢复               | 上述集成检查与受影响的恢复回归                             | 真实 macOS 听写、取消/重试及跨 App 注入，记录具体模式/失败条件           |
| 快捷键、设备、注入、原生窗口 | 原生 build、相关测试与主平台实测                           | 按影响验证录键恢复、预览交接、焦点、权限、多屏/Space；不可访问项目写未测 |
| 仅影响 Rust 测试/helper      | Cargo test 与 clippy --all-targets                         | 不因测试需要扩大生产 API                                                 |

- 新功能或 bug 修复覆盖可观察行为、失败边界；不写只镜像实现或为低影响文案变更凑数量的测试。
- 修改 pipeline/recovery 要核对：转写复用；模式/提示词/选区快照；取消后迟到成功或错误；旧定时器不隐藏新胶囊；终态音频释放。影响相关路径时补对应回归，已有覆盖复用。
- macOS 是本地开发主平台，不要求贡献者先拥有 Windows 机器。影响 Windows 的代码交由既有 CI 编译/测试，并注明系统交互是否实测；CI 尚未运行也如实记录。
- 单测、模拟 IPC、构建、真实系统行为是不同证据。记录本轮命令/退出结果、手动步骤和未测原因；环境受限的完整验收保持开放，不用旧日志代替。
- 涉及系统凭据的 ignored 测试不能当作“已通过”；不为完成验收修改作者真实历史、清空配置、重置系统授权或输出真实 secrets。

## 规范维护与参考

- CONTRIBUTING 维护开发/提交步骤；本文维护工程细则；ARCHITECTURE 维护职责/依赖；STATUS 维护实际进度；OpenSpec 维护当次目标、取舍和验收。相同规则链接到所属文档，避免多处复制。
- 新增约定与实际修复同步提交；遇到文档/实现冲突，先识别既有行为与已批准决策，再修正过期描述。不能因为代码已经不同就自行改变产品合同。
- 本机 AGENTS/CLAUDE 可附加协作约束；公共工程规则和必要开发步骤应纳入 Git。不要为了公开规范把含个人信息或本机配置的文件全部提交。
- `pnpm check:engineering` 自动检查源码规模，以及 CONTRIBUTING、ENGINEERING、ARCHITECTURE、两份 README 的 Markdown 格式与本地相对链接/锚点；直接读取文件，不受 Git ignore 影响。不联网检查外链，不默认格式化历史进度/验收材料；额外 Markdown 可通过命令参数指定。
- 每个 clone 通过 `pnpm hooks:install` 显式启用静态检查 hook，安装器拒绝覆盖已有用户 hook/config；`pnpm install` 不自动安装。具体检查与手动验收命令见 CONTRIBUTING。
- lint/format 等可自动检查的规则交给现有工具；语义、注释准确性和真实系统验收仍需审阅。新增工具或严厉化旧规则另作切片确认。

本规范参考下列资料并按 Audie 规模取舍，不要求照搬它们的目录、库或全部流程：

- [Handy 贡献指南](https://github.com/cjpais/Handy/blob/main/CONTRIBUTING.md)：同类应用的开发及音频/设备验证。
- [Bulletproof React 项目结构](https://github.com/alan2207/bulletproof-react/blob/master/docs/project-structure.md)：功能归属和单向依赖。
- [Rust API Guidelines 文档约定](https://rust-lang.github.io/api-guidelines/documentation.html)：错误、安全前提及接口说明。
- [Zed PR 模板](https://github.com/zed-industries/zed/blob/main/.github/pull_request_template.md) 与 [UI 检查清单](https://github.com/zed-industries/zed/blob/main/docs/src/development/ui-checklist.md)：自查及真实验证证据。
