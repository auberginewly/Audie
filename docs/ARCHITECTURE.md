# Audie 代码导航

2026-11-11 按已批准的职责整理完成。沿一次听写读代码：

工程细则见 [ENGINEERING.md](ENGINEERING.md)，开发及验证命令见 [CONTRIBUTING.md](../CONTRIBUTING.md)。

```text
Platform 热键回调
  → hotkeys（注册/录键/恢复）
  → pipeline/recording（权限、选区、开始/结束采集）
  → pipeline/processing（转写、润色、注入、历史）
  → pipeline/recovery（取消、重试、终态清理）
  → StateMachine event → 前端 dictation 镜像
```

## Rust

| 位置                                       | 职责                                                                                   |
| ------------------------------------------ | -------------------------------------------------------------------------------------- |
| src-tauri/src/lib.rs                       | 插件、40 个 command 注册、Manager 启动装配                                             |
| commands/                                  | 按 settings/dictation/history/providers/audio/permissions/secrets/hotkeys 提供 UI 入口 |
| settings/                                  | types/defaults/catalog、normalize/validation/patch、TOML 读写及旧 JSON 迁移            |
| hotkeys/                                   | 注册、重绑、设置录键时暂停及恢复，全都进入同一 pipeline                                |
| pipeline/context.rs                        | 当前录音模式/提示词/引用、LastTake、generation 和 managed slots                        |
| pipeline/config.rs                         | 从同一 settings loader + 按需 Keychain 读取构造 ASR/LLM 配置                           |
| pipeline/recording.rs、streaming.rs        | 录音生命周期、流式会话交接                                                             |
| pipeline/processing.rs、recovery.rs        | 正常/恢复共用处理路径、旧结果隔离和终态收尾                                            |
| pipeline/events.rs、state.rs               | 原 event payload 和合法状态转换                                                        |
| managers/audio/                            | 采集、录音会话、预览、电平、缓冲保险丝                                                 |
| managers/history/                          | SQLite 存储、查询、统计；Manager API 不变                                              |
| managers/{transcription,enhance,inject}.rs | 保留原有 provider/注入职责                                                             |
| asr/、llm/、provider_test/                 | provider adapter、请求/清理、连接/模型查询                                             |
| platform/                                  | OS 边界；macOS NSPanel/多屏/热键/注入/钥匙串；Windows 保留既有实现                     |

settings 不调用 commands/hotkeys。pipeline 调用 Manager 与 Platform，平台实现不读取 LastTake/generation。原生隐藏真正执行前由 pipeline 再查 generation + Idle。

### 依赖边界

- lib.rs 组装对象并注册命令；业务模块不反向调用装配入口。
- commands 按功能调用 settings、hotkeys、pipeline 或 Manager。settings 只拥有数据、规则、迁移与持久化，不调用 command handler 或控制录音。
- hotkeys 进入唯一 pipeline；pipeline 使用既有 Manager 和 Platform，不依赖前端 command。Manager/provider 不通过 commands 读取设置。
- 平台系统调用及生产代码的 target_os 分支收敛到 platform；平台不读取听写任务缓存、generation 或决定恢复策略。
- 共用类型按业务归属放置，跨子模块仅开放必要的可见性；不为消除一个类型引用增加新层。

## 前端

src/features/{dictation,history,home,providers,text,hotkeys,audio,permissions,settings,onboarding} 各自收拢 UI、hook、schema 和纯逻辑测试。App.tsx 负责导航和功能组合；components/{ui,shell} 为通用界面，i18n/ 管语言元数据和按功能拆分的文案。

Rust 是听写权威；两个 WebView 独立接收事件，Zustand 只作镜像。home 单向消费 history 数据合同，useSettings 保留原五请求 Promise.all 加载语义。Text 仍只编辑三份固定正文，没有多模板管理或循环键。

- App、settings 和 onboarding 等组合入口可以组装各功能组件；不把组合逻辑反向放进底层 schema。
- 允许按用途消费其他功能的稳定合同，例如 home 消费 history 的统计 schema、settings 消费 providers/audio 的 schema；不为方便直接读取别人的内部状态或形成运行时循环。
- 组件需要配置类型时用 type-only 引用；类型引用与运行时依赖分开检查。
- ui/shell、语言基础设施等公共模块不反向依赖具体功能；单一功能的实现不搬入万能 shared/utils 桶。

## 测试与验证

Rust 单测在所属模块的 tests.rs / tests/，通过 cfg(test) 接入；前端纯测试在功能 `__tests__/`，发布脚本测试留根 tests/。pnpm test 同时发现两处，零发现和失败均返回非零。业务和测试源码分别检查 400 物理行目标，不为凑行数删注释。

验证与真实系统待测边界见 [本轮集成记录](../openspec/changes/organize-dictation-core/evidence.md)。没有新增依赖、第二套状态、agent 或后端服务。
