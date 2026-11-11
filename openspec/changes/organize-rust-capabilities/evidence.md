# 实施记录（2026-11-11）

## 范围与目录

本片按已批准 S6a–S6c 实施，未新增依赖或能力；由主任务完成设置、command 注册和原生应用集成。

| 原位置 | 迁入职责 |
| --- | --- |
| managers/audio.rs | audio/mod.rs 保留 Manager/私有共享状态；recording.rs 会话开始停止；capture.rs CPAL 线程和设备枚举；monitor.rs 预览与电平 emitter；buffer.rs 采样归一化/保留保险丝/有界队列；buffer/tests.rs |
| managers/history.rs | history/mod.rs 保留 Manager API/数据；storage.rs 原建表与兼容 ALTER；queries.rs 条目 SQL；stats.rs 原聚合 SQL；tests.rs |
| llm/mod.rs | mod.rs trait/构建/local 判定；openai_compatible.rs 请求与错误；sanitize.rs 输出清理；各自就近测试 |
| provider_test.rs | mod.rs 请求/返回合同与共用 key/错误规则；connection.rs test_provider；models.rs 模型列表/本地探测；各自就近测试 |
| asr 内联测试 | asr/tests.rs、glm/tests.rs、aliyun/{client,codec}/tests.rs、stepfun/client/tests.rs、doubao/{client,codec,config}/tests.rs |
| asr/doubao/client.rs | 配置/鉴权头移既有 config.rs，PCM 编码移既有 codec.rs；client 对 DoubaoAuth/DoubaoStreamConfig 保留原类型路径 reexport |

ASR 其余 adapter 分离测试后已小于 400 行且仍为单一 provider 职责，因此保留实现，不额外机械拆分。

## 保持不变的边界

- AudioManager/HistoryManager 的公开方法、参数/返回值和调用顺序原样；录音锁、线程、CPAL drop/最终帧、512 MiB 保险丝、有界队列溢出行为不变。
- History SQLite 路径、DDL、SQL、时间窗口、计数口径及 event 不变，无数据迁移。
- LLM/ASR 网络协议、鉴权值、请求体、错误文案、超时、同步/异步方式与 PCM 算法不变。
- provider command invoke 名和参数不变；注册路径为 provider_test::connection::test_provider 与 provider_test::models::{list_provider_models,discover_local_llm}。
- Audio/History 设置读取由 crate::commands::load_settings 改为 crate::settings::load_settings；provider 注册表/验证器从 settings 消费。
- 只为真实跨子模块使用增加 pub(super)；共享会话字段仍在私有 audio 根作用域。测试挂所属私有子模块，无测试专用公开业务接口。

## 本次自动验证

- 搬迁前保存 18 个原始能力源码文件；函数/类型词法审计保留字符串与注释的原始字节，仅忽略其外空白并允许 commands→settings loader 路径替换。原 289 个函数正文全部保留（包含 129 个测试）；129 个测试属性相同；46 个 struct/enum 正文相同（唯一作用域差别为 CapturePlan/CaptureThreadArgs 的必需 pub(super) 字段）。缺失/新增均 0。
- 45 个能力 Rust 源码文件全部 <=400 物理行，最大 asr/doubao/codec.rs 387 行；没有删除必要注释/压缩代码达标。
- rustfmt 对所有 owned 路径检查退出 0；主集成亦已执行 cargo fmt。
- cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings：退出 0。存在既有 block v0.1.6 future-incompatibility 提示，不是本片新增编译告警。
- cargo test --manifest-path src-tauri/Cargo.toml：退出 0；lib 200 passed / 1 ignored / 0 failed（201 discovered），main 0、doc-tests 0。唯一忽略项仍为会触碰用户 Keychain 的原 smoke 测试。
- 用该次构建出的测试 harness --list 核对 201 项，原 owned 129 项身份（忽略模块前缀）全部被发现，无遗漏。主任务负责最终完整 Cargo 清单与全局对照。
- git diff --check：退出 0。
- openspec validate organize-rust-capabilities --strict --no-interactive：通过；skip_specs: true。

## 未验证

本执行单元没有进行麦克风/预览/跨 App 注入、provider 真实联网、原用户历史操作、Windows 编译或系统行为验收。原生构建及真实应用证据由主集成记录；不把源码审计或单元测试视作这些场景通过。任务 6 保持未勾选，不归档 change。

## 集成补记

入口/流程先以 00602d5 单独提交，使用临时 settings reexport 兼容本片旧消费者，并对该独立 index 快照跑过 Cargo test（200 passed / 1 ignored）及 clippy -D warnings。本片接上直接 settings 消费与实际 provider command 路径，删除全部临时转发。

主集成额外运行 clippy --all-targets -- -D warnings，通过。为原 history 测试 insert_enhanced fixture 增加局部 too_many_arguments allow 与原因注释，函数正文/测试断言保持。原生 build 亦通过，真实系统待测边界仍未代勾。
