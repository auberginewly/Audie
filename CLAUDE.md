# CLAUDE.md

> 这个文件是给 Claude Code / AI 协作用的工程约束。它存在的唯一目的：**防止项目变成一个我自己驾驭不了的大工程而烂尾。**
> 每次开新 session，按顺序读：
> 1. **本文件**（协作规则 + 末尾「当前进度」）→ 明确这次只做哪一个切片
> 2. [PROJECT_SPEC.md](PROJECT_SPEC.md)（产品定义 + 架构 + 当前阶段的 command/event/错误大类）→ 拿到接口语义
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
4. [ ] + 剪贴板法注入光标
5. [ ] + 设置持久化

**现在在做：** P0.4 待开（剪贴板法注入光标处：保存剪贴板 → 写入文本 → 模拟 Cmd+V → 恢复剪贴板）。
> 注：VAD（自动断句）已从 P0 移除——产品方向是 fn 单击 toggle 手动控制录音起止（P3）+ 边录边传流式输出（P2），与「系统自动决定何时结束」对立，故不做 VAD。当前「按住快捷键」是 P0 临时过渡方案。

P0.3 坑：**Groq 在中国大陆被地区限制**，直连返回 `403 Forbidden`（裸 message，不是 invalid_api_key），开代理/虚拟网卡后正常。后续考虑加国内可直连 provider（硅基流动等）。

P0.2 收尾遗留：
  - 3 根柱状体只是占位，typeless 那种 N 根条形阵列留到 P3 体验打磨
  - 麦克风选择 (`list_microphones`) 是 P0.5 的活，当前固定默认设备
  - cpal 失败时（无权限 / 无设备）目前只 log，UI 上柱状体不动；P0.6 才把 `AppError::Permission` / `Device` 推到 `error` event 让 overlay 变红
  - Info.plist 是否被 Tauri 2 bundle 真正合进 .app 还没在 release build 上验证过；P4 打包时复核