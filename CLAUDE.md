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

- [ ] **P0** 跑通核心链路（复刻 Handy 最小集）：快捷键→录音→批量转写→剪贴板注入→设置持久化
- [ ] **P1** 变成 typeless：provider 抽象 + AI 润色 + BYOK + key 存 keychain
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

**现在在做：** P0 收尾——P0.7 待开（剩下的错误态 + 注入失败兜底，见下面遗留清单）。
> 注：VAD（自动断句）已从 P0 移除——产品方向是 fn 单击 toggle 手动控制录音起止（P3）+ 边录边传流式输出（P2），与「系统自动决定何时结束」对立，故不做 VAD。当前「按住快捷键」是 P0 临时过渡方案。

P0.6 遗留 + 设计决策：
  - **Esc 取消逻辑本切片不做**：原计划 §3.9「Esc 中断录音不注入」实现到一半被砍掉。原因：用户决定换成更彻底的「胶囊上 ✓/✗ 可点按钮 + 撤回 + 转录文字仍保留」交互模型，与 toggle 触发（fn 键）配套，是项目最终手感。已记进 [PROJECT_SPEC.md](PROJECT_SPEC.md) §5 P3「Overlay 交互 + 控制模型（未来方向）」。此前砍掉的 Esc 实现（Handy 风格动态注册全局 Escape + `Recording→Cancel` 丢弃音频）见 git 历史的 reset 之前；未来做 ✓/✗ 时不必复用，重新设计。
  - 由此连带：`Platform::unregister_hotkey` / `HotkeyRegistry::remove` 也未加入——做 ✓/✗ 时若需要动态键再加。

P0.7 待做（原 P0.6 没收下的尾巴）：
  - **AirPods 静音检测**（开流前 N 毫秒 peak 一直 0 → `AppError::Device("麦克风没声音，检查蓝牙耳机是否切到通话模式")` 让胶囊变红，比静默吐 "Thank you." 强）
  - **cpal 设备失败上报**：`start_capture` 失败现在只 log（`AppError::Device`/`Permission`），要走 `error` 事件让 overlay 变红
  - **注入失败保留剪贴板**：当前注入失败也跑 120ms 后恢复旧剪贴板，把转写文字也清掉了，违反 §3.7「注入失败文字留在剪贴板」。要在 `inject_text` 检测 `CGPreflightPostEventAccess()` 为 false → 不恢复剪贴板 + `AppError::Permission` 让胶囊变红
  - **网络/Provider 友好文案**：Groq 403（中国大陆地区限制）、key 无效、超时等，目前文案是裸 error message，要分类成 §3.7 的大类

P0.5 收尾遗留：
  - **麦克风选择持久化推后**（原 §3.1/§3.9 写在 P0.5）：本切片只做快捷键。麦克风的「选 + 存」一起挪到以后的设置页做，故 §3.9「麦克风选择保留」这条 P0.5 不覆盖。
  - 快捷键只给 3 个预设下拉（`Ctrl+Shift+Space`/`Alt+Space`/`Ctrl+Alt+Space`），自由录键控件是 P3 设置页的活。
  - 自定义 command（`get_settings`/`update_settings`）走默认放行，未进 capability；store 只在 Rust 侧用 `StoreExt`，前端经 command 读写不直接碰 store JS API。

P0.5 期间发现、留给后续切片的 AirPods 静音坑：
  - **症状**：偶发录音柱状体完全不动 + Whisper 吐 "Thank you."（经典低音量幻觉）。
  - **机制**：AirPods 麦克风走蓝牙 HFP/SCO，「有 app 真用麦」时 macOS 才切到 HFP；没切前设备「存在且被选中」但读出来全零。期间被其他 app（Zoom/微信/网页）抢过麦或 AirPods 短暂断连重连，系统默认输入会悄悄换，我们这条 cpal 流握的还是老设备 → 静音。
  - **临时绕开**：开 App 前在系统设置改输入为内置麦；或先在 QuickTime 录一秒逼 AirPods 切 HFP 再用。
  - **P0.6 要做**：开流后前 N 毫秒 peak 一直为 0 → 发 `AppError::Device("麦克风没声音，检查蓝牙耳机是否切到通话模式")` 让胶囊变红。比起静默吐 "Thank you." 强得多。
  - **未来设置页要做**：`list_microphones` 显式选设备，不无脑跟系统默认走；监听设备变化事件，被换设备时主动重建 cpal 流或报错。

P0.4 坑 + 遗留：
  - **CGEvent 合成 Cmd+V 需要 Accessibility 权限**。没授权时 `CGEvent::post` 不报错、按键被 macOS 静默丢弃 → 粘贴无声失败（`CGPreflightPostEventAccess()` 可查，true 才能 post）。dev build 跑的是 `target/debug` 未打包二进制，需在 系统设置→隐私与安全性→辅助功能 手动给它打勾。权限引导是 P0.6（错误态）/ P3（onboarding）的活。
  - **连带 bug，留给 P0.6**：注入失败时当前仍照常 120ms 后恢复剪贴板，把转写文字也清掉了，违反 §3.7「注入失败文字留在剪贴板」。P0.6 做无权限错误态时一并修：检测 `CGPreflightPostEventAccess()` 为 false → 不恢复剪贴板 + 发 `AppError::Permission` 让胶囊变红。

P0.3 坑：**Groq 在中国大陆被地区限制**，直连返回 `403 Forbidden`（裸 message，不是 invalid_api_key），开代理/虚拟网卡后正常。后续考虑加国内可直连 provider（硅基流动等）。

P0.2 收尾遗留：
  - 3 根柱状体只是占位，typeless 那种 N 根条形阵列留到 P3 体验打磨
  - 麦克风选择 (`list_microphones`) 是 P0.5 的活，当前固定默认设备
  - cpal 失败时（无权限 / 无设备）目前只 log，UI 上柱状体不动；P0.6 才把 `AppError::Permission` / `Device` 推到 `error` event 让 overlay 变红
  - Info.plist 是否被 Tauri 2 bundle 真正合进 .app 还没在 release build 上验证过；P4 打包时复核