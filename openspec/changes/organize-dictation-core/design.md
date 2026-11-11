# Design

## Decisions

继承总方案 Manager + Command/Event + Platform，不引入新的架构模式。

- lib.rs 是插件、command 注册和 Manager 装配根；hotkeys → pipeline → managers / Platform。
- commands/settings 负责原有副作用顺序；settings 不调用 commands/hotkeys。所有消费者用同一 loader。
- pipeline/context 保存原 LastTake、各 managed slot；ActivePromptSlot 仍为独立新类型，避免 Tauri 按具体类型索引时和 RewriteSourceSlot 冲突。
- processing/recovery 保留锁范围、generation 校验、转写复用、模式/正文/引用快照。系统粘贴已发生后不可撤回的原边界不变。
- 原生窗口隐藏由 pipeline 派发到主线程，真正执行时再次核对 generation + Idle，再调用 Platform。平台不读取业务 take。
- 仅业务协作需要的 pub(super)/pub(crate)；测试访问父模块，不为测试新增公共 API。

## Files

- settings/{types,defaults,catalog,normalize,patch,validation,persistence}.rs + tests/
- commands/{settings,dictation,history,providers,audio,permissions,secrets,hotkeys}.rs
- hotkeys/{mod,registration,capture}.rs
- pipeline/{mod,context,events,config,recording,streaming,processing,recovery}.rs + tests/
- platform/{overlay.rs,macos/overlay.rs,macos/hotkey/} + 原有模块的就近 tests.rs

## Validation

对照迁移前全部 Rust 测试身份及正文/属性、40 个 command 注册；全量 Cargo test/fmt/clippy、前端 F、原生 build。跨 App、真实录音、多屏/Space、Windows 另列未实测，不用静态检查冒充系统验收。
