# CLAUDE.md

> 这个文件是给 Claude Code / AI 协作用的工程约束。它存在的唯一目的：**防止项目变成一个我自己驾驭不了的大工程而烂尾。**
> 每次开新 session，按顺序读：
> 1. **本文件**（协作规则 + 末尾「当前进度」）→ 明确这次只做哪一个切片
> 2. [PROJECT_SPEC.md](PROJECT_SPEC.md)（产品定义 + 架构 + 当前阶段的 command/event/错误大类）→ 拿到接口语义
> 3. [docs/SESSION_PLAYBOOK.md](docs/SESSION_PLAYBOOK.md) 末尾「中断信号」一节（其余文档也都在 `docs/`，别再问路径）
>
> 两份冲突时以本文件为准。

---

## 项目是什么

跨平台（先 macOS，后 Windows）语音输入工具，typeless 的开源平替。
按下快捷键说话，松手后文字自动出现在当前光标处。BYOK（用户自带 API key），客户端直连，无后端服务器。

**一句话定位：Windows/跨平台上的 typeless 平替 + 真流式上屏。** 不是"更好的 Voxt"，是错位竞争。

### 明确不做（Non-Goals · 防范围蔓延）
做着做着想加新东西时，先对照这个清单。要改清单，停下来跟我确认，不许 AI 自己加。
- ❌ 会议转录 / 长音频 / 字幕导出（那是另一类产品，OpenWhispr 的方向，不碰）
- ❌ 语音命令 / agent / 执行操作（想做 agent 是**另一个项目**，绝不混进来）
- ❌ 移动端
- ❌ 实时翻译（typeless 有，但不是核心价值，砍）
- ❌ 说话人分离 / diarization
- ❌ 账号体系 / 云同步 / 计费（BYOK 客户端直连就是为了避开这些）
- ❌ 自建任何后端

核心价值就一句：**按快捷键说话 → 干净的文字出现在光标处，快**。其余一律先放进 Non-Goals。

---

## 最重要的三条规则（违反了就会重蹈覆辙）

### 1. 架构决策由我做，AI 只在已定架构内写实现
前两个项目烂尾，根因是让 AI 替我做了架构决策，生成出我看不懂、改不动的复杂度。
- 任何"要不要加一层 / 用什么模式 / 怎么组织模块"的决定，停下来问我，不要自作主张。
- 写代码前先说清楚要改哪些文件、为什么、和现有结构的关系，我确认后再写。
- 不要引入我没听说过的库或抽象。要引入，先解释替代方案和取舍。

### 2. 先读再写
实现任何系统层功能前，先参考已验证的开源实现，不要凭空 vibe：
- **主骨架照着复刻 `cjpais/Handy`**（~2.3w star，Tauri2 + Rust + React，同栈，明确为 fork 设计）。它的 Manager 模式、Command-Event 架构、pipeline、状态流就是本项目模板。先 clone 跑通，读 AGENTS.md 和 lib.rs。
- provider 抽象、词典、上下文 prompt 等**产品思路**参考 `hehehai/voxt`（Swift，**只看思路不抄代码**）和 OpenWhispr（BYOK 设计）。
- 同目标 Tauri 实现交叉验证：`voquill/voquill`、`kstonekuan/tambourine-voice`。
- 写系统层（快捷键/注入）时，明确让 AI"先看 Handy 对应实现，按它的模式来"，再动手。

Handy 是纯本地、无润色、无 BYOK——润色 + 流式 + BYOK 是本项目在它之上加的差异化。

### 3. 每个里程碑独立可用、可演示
防烂尾的核心机制。每步做完都要能"按下快捷键 → 看到结果"，哪怕功能很少。
- 不允许出现"做了一半、几个模块都半成品、跑不起来"的状态。
- 宁可功能少，也要每步都是 working software。
- 一个切片没收尾（能跑 + 能演示 + commit 干净），不开下一个。

---

## 技术栈（已定，不要改）

- **框架**：Tauri 2
- **前端**：React + TypeScript + Zustand（状态机）
- **后端/热路径**：Rust（录音 → 转写 → 注入 全在 Rust，前端只收状态事件）
- **样式**：Tailwind + **daisyUI**。克制、功能性优先，不要花哨。选克制主题（nord 或自定义，圆角调小、色板收敛），别用默认那种圆润活泼味。早期 UI 是消耗品，视觉打磨推到 P3。
- **包管理**：pnpm（前端）、cargo（Rust）

---

## 架构铁律

### 整体架构（复刻 Handy 的 Manager + Command-Event 模式）
- **Manager 模式**：核心功能拆成 manager（AudioManager / ModelManager / TranscriptionManager / 新增 EnhanceManager、InjectManager），启动时初始化，挂在 Tauri state 上。
- **Command-Event**：前端→后端走 Tauri command，后端→前端走 event（状态变化、partial transcript、音量）。
- **状态流**：Zustand → Tauri Command → Rust State → tauri-plugin-store 持久化。
- **入口写 lib.rs 不写 main.rs**（Tauri 约定，main.rs 只调 run()）。

### 平台抽象层（Handy 没显式分，必须加，从第一天就要有）
所有平台相关系统调用收敛到一个 trait 后面，不允许 `#[cfg(target_os)]` 散落各处。
```rust
trait Platform {
    fn register_hotkey(&self, ...) -> Result<HotkeyId>;
    fn inject_text(&self, text: &str) -> Result<()>;
    fn store_secret(&self, key: &str, val: &str) -> Result<()>;
    fn read_secret(&self, key: &str) -> Result<String>;
}
```
macOS 实现先写，Windows 后补。录音、流式 pipeline、provider、整个前端都平台无关（≈70% 代码）。

### Provider 抽象层
ASR 和 LLM 各自是可替换 trait，加新引擎 = 实现一个 adapter，不改其他代码。
```rust
trait AsrProvider {
    async fn transcribe(&self, audio: AudioStream) -> Result<TranscriptStream>;
}
trait LlmProvider {
    async fn enhance(&self, text: &str, prompt: &str) -> Result<String>;
}
```
LLM 走 OpenAI-compatible 接口（base_url + key + model 三字段），用户可填 OpenAI / DeepSeek / 硅基流动 / Ollama 等。

### 数据流（单向，状态机驱动）
```
hotkey 按下 → RECORDING → (音频 chunk 流式上传) → PROCESSING → 注入 → IDLE
```
前端是状态机的镜子，只渲染 Rust 发来的事件，不持有业务逻辑。

### Overlay 状态机（唯一要先想清楚的 UI）
屏幕底部胶囊，透明、点击穿透、置顶，数据全来自 Rust event：
IDLE(不显示) → RECORDING(波形+实时 partial 文字) → PROCESSING(脉冲) → SUCCESS(闪一下淡出) / ERROR(变红带消息) / CANCEL(Esc 直接淡出不注入)。
P0 阶段用最朴素的"录音中…/处理中…"文字占位，动效手感留到 P2/P3，且要在代码里搭空壳手动切状态来调，不画静态稿。

---

## 安全 / 隐私底线
- API key 存系统 keychain（macOS Keychain Services / Windows Credential Manager），**绝不明文写配置文件**。
- macOS keychain 实现参考 Voxt：用底层 `SecItemCopyMatching` / `SecItemAdd` / `SecItemUpdate` / `SecItemDelete` 管 generic-password item，service 用 `com.audie.app.secure-storage`，写入设置 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`；`has_secret` 只做 presence check（不带 `kSecReturnData`，不读明文），只有 provider test / ASR / LLM 真需要 key 时才 `read_secret`；保存一次后重启应静默读取，不能每次打开设置页都弹系统密码。
- 不自建任何后端，音频和 key 只在用户设备与用户自己的 API 之间流动。
- 导出配置时敏感字段用占位符替换。

---

## 代码风格

**Rust**
- `cargo fmt` + `cargo clippy`（CI 里 `-D warnings`，本地提交前跑过）。
- 错误用 `anyhow`（应用层）/`thiserror`（库层定义错误类型），热路径**不准 `unwrap()`/`expect()`**，要处理就显式处理。
- 命名：模块 snake_case，类型 CamelCase，manager 统一 `XxxManager`。
- async 用 tokio，阻塞操作（whisper 推理、文件 IO）放 `spawn_blocking`，别堵事件循环。

**TypeScript**
- 严格模式（`strict: true`），**禁止 `any`**，外部输入（设置、API 响应、Rust event payload）一律 Zod 校验后再用。
- ESLint + Prettier，CI 里 lint 不过不准合。
- 组件函数式 + hooks，业务逻辑抽进 hook/store，组件只管渲染。

**提交**
- Conventional Commits：`feat:` `fix:` `refactor:` `chore:` `docs:`。粒度小，一个 commit 一件事。
- message 说清"为什么"，不是"改了什么"（diff 已经显示改了什么）。
- 这是开源 + portfolio 项目，commit history 本身会被面试官看，保持干净。

**注释**：解释 why 不解释 what。复杂的系统层（CGEventTap、剪贴板时序）写清楚为什么这么做、坑在哪。

---

## CI/CD（分层上，别一次到位）

**本地（P0 就配，便宜）**
- 提交前钩子（lefthook 或简单 git hook）：`cargo fmt --check` + `cargo clippy` + `pnpm lint` + `pnpm tsc --noEmit`。挡住低级错误。

**CI — PR 检查（P0/P1 阶段就值得有）**
- GitHub Actions，PR 触发：fmt check、clippy（`-D warnings`）、ts lint、typecheck、`cargo test`、`pnpm build`（确保前端能编译）。
- 只做"能不能过"，**不做多平台打包**——那个慢且贵，早期不需要。
- 跑在 macOS runner 上即可（你主力平台）。

**CD — 发布（P4 才做，别提前）**
- 用 `tauri-action`，打 tag 触发，多平台构建产物（macOS dmg、Windows msi、Linux deb/AppImage）。
- Tauri updater 签名：minisign 格式，公钥放 `tauri.conf.json` 的 `plugins.updater.pubkey`，私钥进 GitHub Secrets（**绝不进仓库**）。Handy 就是这套，可直接参考。
- macOS 公证（notarization）等真要公开分发时再搞，自用 / 早期开源可先跳过，让用户右键打开。

> 原则：CI 早上（防 main 烂），CD 晚上（app 立住了再说）。别在 P0 就纠结代码签名和自动更新。

---

## 怎么和我协作（vibe 工作流）

- **一次一个切片**，一个 session 尽量对应一个 commit。别铺大。
- **写前先出方案我拍板**：改哪些文件、为什么。要引入新库/新抽象先停下问我。
- **小 diff，我要看懂**：看不懂、讲不清的代码不准合进去。宁可拆小、解释清楚。
- **"做完"= 能跑 + 能演示**，不是"AI 说写好了"。每个切片跑起来验证。
- **卡住就回滚**：开始"修 bug 引新 bug、补丁摞补丁"时停手，`git reset` 回上一个干净 commit，把切片拆更小再来。不要顺着螺旋往下走。
- **频繁 commit**，git 就是 undo。切片收尾：跑通 → commit → 更新下方「当前进度」。

一句话：我负责架构和取舍，你负责在我画好的框里加速搬砖。我能讲清每个决策，这项目才是我的。

---

## 当前进度
> 每次 session 更新这一节，让下一次 session 知道做到哪了。

- [x] **P0** 跑通核心链路（复刻 Handy 最小集）：快捷键→录音→批量转写→剪贴板注入→设置持久化
- [x] **P1** 变成 typeless：provider 抽象 + AI 润色 + BYOK + key 存 keychain
- [ ] **P2** 真流式上屏（差异化核心）：流式 WebSocket ASR + 实时 partial 预览 + 润色并行
- [ ] **P3** 体验打磨：词典 + 上下文 prompt + onboarding + fn 键
- [ ] **P4** 跨平台：Windows 适配 + CI 多平台构建

**P0 切片（每个独立 commit，不收尾不开下一个）：**
0. [x] 项目脚手架（Tauri 2 + React + TS + Tailwind v4 + daisyUI nord + zustand + zod；目录骨架按 SPEC §7 一次摆好；fmt/clippy/tsc/vite build 全绿）
1. [x] 快捷键 → 空胶囊弹出/消失（默认 `Ctrl+Shift+Space` 按住模式；主窗 + overlay 两 webview；`HotkeyRegistry` 解耦 plugin handler 与 callback；状态机 IDLE↔RECORDING 由 Rust 发 `state-change` 事件驱动前端）
2. [x] + 按住录音 + 波形（cpal 默认输入设备；AudioManager 双线程：cpal Stream 在独占线程里 own（`!Send`），emit 线程 ~30 FPS 推 `audio-level` peak；overlay 渲染 3 根柱状体，gamma 0.6 把语音峰值放到可见量级；`src-tauri/Info.plist` 加 `NSMicrophoneUsageDescription` 走 macOS TCC）
3. [x] + 松手转写打印控制台（`AsrProvider` trait + `asr/groq.rs` adapter；multipart 上传 + 手写 16-bit PCM WAV 编码；key 从 `GROQ_API_KEY` env 读，P1 才进 keychain；AudioManager 录音时顺带攒 f32 样本 buffer，`stop_capture` 交出整段 `AudioData`；松手走 `Processing` → 转写线程 → 打印 + `final-transcript` 事件 → `Success`/`Error` → `Idle`；reqwest blocking + rustls-tls，在独立线程跑不引 async runtime）
4. [x] + 剪贴板法注入光标（`InjectManager`（薄编排层）→ `Platform::inject_text`（macOS 实现整套剪贴板法，OS 细节都在 platform 层）；剪贴板存写恢复用 `tauri-plugin-clipboard-manager`，Cmd+V 用 `core-graphics` CGEvent 合成 keycode 9 + Command flag；写后 20ms、粘后 120ms 再恢复旧剪贴板；转写成功 → 发 `final-transcript` → 注入 → `Success`，注入失败走 `enter_error`）
5. [x] + 设置持久化（`tauri-plugin-store`；`commands.rs` 新增 `get_settings`/`update_settings`，store 直接承载设置不进 manager；快捷键从存档读、改键时注销旧的→注册新的→存盘；`build_hotkey_callback` 抽成可复用函数，从 app.state 解析 manager 而非捕获 clone，改键时原样重建；前端主窗预设下拉，label 用 macOS 符号 ⌃⇧⌥ 显示、value 仍是 global-shortcut 认的 combo 串）
6. [x] 错误态底座 + 麦克风权限红胶囊（`AppError` 加 `code()`/`message()`/`recoverable()`，`error` 事件改发 `ErrorPayload{code,message,recoverable}` §3.6；overlay 从「松手即隐」改为「回 IDLE 才隐」（`settle_to_idle` 统一出口），让 PROCESSING/ERROR 用户看得见；前端 `AppErrorSchema` Zod 校验后入 store，Capsule ERROR 态变红显消息 2.5s；`Platform::ensure_microphone_permission` 用 `tauri-plugin-macos-permissions`（Handy 同款，request 触发 TCC 弹窗→check 读结果），按键前 gate，未授权直接 `Idle→Error` 不录音）
7. [x] 错误态收尾 + 蓝牙/Continuity 输入设备绕开（注入：`inject_text` 写完剪贴板后预检 `CGPreflightPostEventAccess`，false → 保留剪贴板返回 `Permission` 不再恢复，并尽力 `CGRequestPostEventAccess` 把 Audie 加进辅助功能列表 §3.7 Inject fallback；cpal：Pressed 路径先 `start_capture` 再 transition，失败走合法的 `Idle→Error`；Groq 错误分类：reqwest `is_timeout`/`is_connect` 友好文案，HTTP 401→`Provider`、403/429/5xx→`Network`、其他 4xx→`Provider`；静音兜底：`is_digital_silence` 用 1e-4 阈值 + 200ms 最低时长，命中发 `Device` 错不烧 Groq；`Platform::preferred_input_device_name` 走 CoreAudio HAL，给 transport 打分——`bltn`/`usb ` = 0、`blue`/`blea`/`ccwd`/`ccwl`/`airp` = 2、其他 = 1，系统默认 score>0 时挑严格更低的设备，避免 AirPods A2DP 静音和 iPhone Continuity 假阳性）

**P1 切片（每个独立 commit，不收尾不开下一个）：**
1. [x] Provider 配置骨架 + 可用 provider 列表（`Settings`/前端 Zod schema 加 `asr_provider` / `llm_provider` / `enhance_enabled` / `enhance_prompt`；`list_asr_providers` 和 `list_llm_providers` 返回 Voxt 风格轻量 metadata：id/title/kind/engine/default_model/requires_key/tags；设置页能读写这些字段；`update_settings` 改为 `patch: SettingsPatch`，仅 hotkey 变化时才重注册快捷键；现有 Groq 默认链路保持默认 `asr_provider = groq`）
2. [x] Keychain secret command（`set_secret` / `has_secret` / `delete_secret` 走系统 keychain；重启 App 后 key 仍可查到；store 文件里不出现 key 明文；手动验收：DevTools 调 `set_secret` 后 `has_secret=true`，重启后仍为 `true`，`delete_secret` 后 `has_secret=false`；临时 `withGlobalTauri` 已移除）
3. [x] BYOK 设置页 + provider test（设置页新增 Groq / OpenAI / OpenAICompatible 三块 key 表单，写入仍走 `set_secret` 到系统 keychain；新增 `test_provider` command 探测 `/models` 可达性，成功/失败在设置页明确反馈；reqwest timeout/connect 归 `Network`，401 归 `Provider`，403/429/5xx 归 `Network`，不联网时给「网络失败」错误）
4. [x] ASR provider 切换（批量 ASR 支持 Groq / OpenAI / WhisperCpp 三个 adapter；切 ASR provider 不用重启；没有配置 key 或模型时走 `Provider` / `Device` 类错误，不 panic；Groq/OpenAI 真实批量 adapter 走 keychain，WhisperCpp 按 SPEC 只接 adapter 骨架不引 whisper.cpp/whisper-rs；`cargo test`、`cargo clippy -- -D warnings`、`pnpm exec tsc --noEmit`、`pnpm build` 通过）
5. [x] LLM 润色链路（`llm::LlmProvider` + OpenAICompatible `/chat/completions` adapter；`EnhanceManager` 按设置开关注入原文/润色文本；OpenAICompatible `base_url` / `model` 持久化，key 仍走 keychain；润色阶段发 `enhance-progress {phase,message}`，胶囊显示「润色中…」/失败兜底提示；润色失败不进 Error，注入转写原文 + 提示「润色失败但已注入原文」；`cargo test`、`cargo clippy -- -D warnings`、`pnpm exec tsc --noEmit`、`pnpm build` 通过）
6. [x] 配置导入导出 + P1 收尾（`export_config` 导出的 JSON 不含 key 明文，敏感字段用 `"<keychain>"`；`import_config` 遇到占位符提示用户重新填 key；完整跑完 SPEC §4.5 验收清单；设置页提供导入导出 JSON 文本入口；导入只写非敏感 settings，遇到 Groq / OpenAI / OpenAICompatible key 占位符提示用户重填；`cargo test`、`cargo clippy -- -D warnings`、`pnpm exec tsc --noEmit`、`pnpm build` 通过）

**P1 收尾修复：**
- [x] Keychain 反复弹窗修复（按 Voxt 思路把 `has_secret` 改成不读明文的 presence check；macOS keychain 从 `SecKeychain::default().set_generic_password` / `find_generic_password` 切到 `SecItemCopyMatching` / `SecItemAdd` / `SecItemUpdate` / `SecItemDelete`；新 service 为 `com.audie.app.secure-storage`，旧 `audie` service 不自动迁移，用户需重填一次 key；`cargo test`、`cargo clippy -- -D warnings`、ignored macOS keychain smoke test 通过）
- [x] Provider test 改成 Voxt snapshot 模型（设置页先用 `has_secret` 做 presence check，已有 key 时通过受限 `get_secret_for_settings` 读入 password input state；`test_provider` 只消费当前表单里的 inline `api_key`，不在测试 command 内 fallback 读 keychain；macOS keychain 底层保持 Voxt 风格 `SecItem* + kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`，不加 `SecAccess/kSecAttrAccess`；`cargo fmt --check`、`cargo test`、`cargo clippy -- -D warnings`、`pnpm exec tsc --noEmit` 通过）

**P2 切片（每个独立 commit，不收尾不开下一个）：**
1. [x] P2 正式 SPEC：接口合同 + 切片表（`PROJECT_SPEC.md` §5 已补 P2.1–P2.6 切片、验收、`partial-transcript`/`final-transcript`/`transcribe_stream` 语义、错误分类和 P2 明确不做范围；本切片只改文档，不写代码、不引入新库）
2. [x] 给 ASR provider 增加流式合同，但先不接真实 provider（`asr::AudioChunk` / `TranscriptDelta` / `TranscriptStream` + `AsrProvider::transcribe_stream` 默认 stub；`TranscriptionManager::transcribe_stream` 暴露合同但不接热路径；`partial-transcript` payload 结构先定义不 emit；现有批量 Groq/OpenAI/WhisperCpp 链路不变；`cargo fmt --check`、`cargo test`、`cargo clippy -- -D warnings` 通过）
3. [x] 豆包二进制 codec + 单测（pure 函数；`src-tauri/src/asr/doubao/{mod,codec}.rs`：build full client request / audio chunk / final negative；parse server response 含 gzip 解压 + 抽 `result.text`；server error frame → `CodecError::ServerError`；14 个 round-trip / 错误分支单测全绿；新依赖 `flate2`；不连网、不动 manager、不动 UI）
4. [x] 豆包配置 UI + keychain + overlay 文案清理（设置页豆包小节 4 字段：AppID/Token 走 keychain、endpoint/resource_id 进 store 明文；**AppID 也当敏感字段进 keychain，对齐 Voxt 的 `SensitiveField` 模型**；新 `asr/doubao/config.rs` 存默认端点/resource_id/keychain key id 常量；`SECRET_KEY_IDS` 加 `doubao_app_id`+`doubao_access_token` → 导出占位、导入提示重填；新增 `doubao_streaming_preview_enabled: bool` 默认 false；豆包小节拆进新文件 `Settings/DoubaoSettings.tsx`（ProviderSettings 已超 400 行）；TS 拆 `SecretKeyIdSchema`(5,广)/`TestProviderKeyIdSchema`(3,窄)；overlay RECORDING 态早先切片已只剩波形、本片无需改；不进 `list_asr_providers`；`cargo fmt`/`cargo test`/`cargo clippy -- -D warnings`/`tsc`/`pnpm build` 通过）
5. [x] AudioManager PCM chunk 实时出口 + dev-only 离线连通性 command（cpal callback 多一路 broadcast，批量 buffer 不变；新 `asr/doubao/client.rs` ws 客户端；官方文档对齐新版 `X-Api-Key` / 旧版 AppID+Access Token、`X-Api-Sequence: -1`、2.0 resource 默认与旧 resource 迁移；dev command `test_doubao_streaming(wav_path)`；新依赖 `tokio-tungstenite` + `tokio-rustls`；未来 provider 代码规范已落 `PROJECT_SPEC.md` §5.2.1）
6. [x] 录音热路径接流式（开关开 + 豆包配好 → 录音时并行送豆包 ws，松手用豆包 final 替代批量 → 现有 `final-transcript`/润色/注入尾巴；开关关或未配 token → 走 P1 批量；流式失败按 §3.7 进 Error 并回 Idle；不 emit `partial-transcript`，不动 overlay；`cargo fmt --check`、`cargo test`、`cargo clippy -- -D warnings` 通过）
7. [ ] P2 收尾（豆包配好默认走流式，未配批量降级；记录 5 次延迟到 SPEC §5.4；同步 `PROJECT_SPEC.md` §5.1 切片表）

**现在在做：** P2.7 P2 收尾（默认流式 + 延迟测量 + SPEC 同步）。

> P2.3–P2.7 切片表已按「松手即出 / 不要滚动字幕 / fn 留 P3」重新设计，详见 `~/.claude/plans/gentle-churning-owl.md`；原 SPEC §5.1 待 P2.7 收尾时同步。

P0 留给后续阶段的事（不是「遗留」，是有意推后的产品决策）：
- **Esc 中断 / 取消语义**：本来 §3.9 P0 要做，已挪进 P3「Overlay 交互 + 控制模型」与 ✓/✗ 按钮 + toggle 触发一起重新设计。当前状态机里 `Cancel` 状态保留但 P0 无入口（见 SPEC §3.2 注）。
- **麦克风设备选择 + `list_microphones` + 设备变化监听**：挪进 P3「设备选择切片」。P0.7 已通过 CoreAudio HAL transport 打分自动绕开 AirPods/iPhone Continuity，escape hatch 留给 P3 设置页。
- **VAD 自动断句永不做**——产品方向是 fn 单击 toggle 手动控制（P3）+ 流式边录边传（P2），与「系统自动决定何时结束」对立。当前「按住快捷键」是 P0→P3 过渡方案。
- **国内可直连 / 真流式 ASR provider**（Groq 在中国大陆 403，P0.7 已给友好提示）：豆包 / 阿里 Paraformer-realtime / Deepgram 等流式 provider 归 P2 `transcribe_stream` 一起设计；P1 只做批量 ASR + BYOK + 润色。
- **波形 N 根条形阵列、自由录键控件、onboarding、Info.plist 打包后复核** → P3/P4 按 SPEC 节奏来。

<!-- 老的 P0.x 遗留清单已折叠进 SPEC §3.9 验收 + §5 P3 / P4 列表，git 历史可查。-->
