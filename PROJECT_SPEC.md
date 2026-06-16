# Audie · Project Spec

> 跨平台语音输入工具的产品 + 技术 spec。每次 session 开场让 AI 先读这份文件 + `CLAUDE.md` 末尾的「当前进度」。
> 这是单一 source of truth：产品定义、架构铁律、P0/P1 接口语义都在这里。`CLAUDE.md` 只放协作规则和切片进度，旧的 `项目规划.md` 已归档（`项目规划-ARCHIVE.md`）。

---

## 〇、如何使用这份文档（给 AI）

1. **读这份 + CLAUDE.md 末尾「当前进度」** → 明确当前 session 只做哪一个切片。
2. **写代码前**：对照 §3 / §4 的 command / event / 错误大类，确认要新增/修改的接口。这些是合同，新增/修改前停下来跟我确认。
3. **写系统层（快捷键 / 注入 / VAD / 流式）**：先去看 §8 列出的 Handy 对应实现，按它的模式来，不要凭空生成。
4. **架构变动**（加一层 / 改 manager 边界 / 引入新抽象 / 换库）：**停手问我**。spec 没写的就不要自己决定。
5. **看不懂、解释不清的代码不准提交**。宁可拆小重来。

> 详细协作规则在 `CLAUDE.md`，这里只点最容易踩坑的几条。冲突时以 `CLAUDE.md` 为准。

---

## 一、产品定义

### 1.1 一句话

**按快捷键说话 → 干净的文字出现在光标处，快。** Windows / 跨平台上的 typeless 平替 + 真流式上屏。

### 1.2 典型用户旅程

1. 用户在任意 App 的输入框（VS Code / 微信 / Notion / 浏览器地址栏）光标停住。
2. 按下全局快捷键（默认 `Alt+Space`）→ 屏幕底部胶囊浮现，开始录音。
3. 边说边在胶囊里看到实时字幕（partial transcript，P2 才有；P0 只显示「录音中…」）。
4. 松开快捷键 → 胶囊变「处理中」→ 转写 + 润色 → 文字注入到光标处 → 胶囊淡出。
5. 用户继续做他原来的事，整个过程 < 1.5s（P2 目标），手不离主键盘区。

**核心体验承诺**：
- **手感**：胶囊出现、消失、状态切换的动效不能卡、不能晃。
- **准确**：默认就能用，不需要看 README 配三天。
- **隐私**：API key 和音频只在用户设备和用户的 API 之间流，作者拿不到任何东西。

### 1.3 差异化定位

| 产品 | 它有的 | 我们对它的差异 |
|---|---|---|
| **Handy**（~2.3w star, Tauri） | 系统层成熟、纯本地 whisper | 加 BYOK 云端 + LLM 润色 + 真流式上屏 + 跨平台分发 |
| **typeless**（闭源, macOS only） | 润色 + 词典 + 上下文 prompt 体验最佳 | 跨平台 + 开源 + BYOK（不锁定云端账号） |
| **Voxt**（Swift, macOS only） | provider 抽象、App Branch 上下文 | 跨平台 + onboarding 简单（不需要喂 AI 看 README） |
| **OpenWhispr**（Electron） | BYOK 思路 | 同栈 Tauri 性能更好 + 真流式 |

> 一句话竞争：**Handy 的工程骨架 + typeless 的产品体验 + 真流式**。

### 1.4 Non-Goals（明确不做）

加新功能前先对这个清单。要改这个清单 → **停下来跟我确认**。

- ❌ 会议转录 / 长音频 / 字幕导出（那是另一类产品）
- ❌ 语音命令 / agent / 执行操作（**另一个项目**，绝不混进来）
- ❌ 移动端
- ❌ 实时翻译
- ❌ 说话人分离 / diarization
- ❌ 账号体系 / 云同步 / 计费
- ❌ 自建任何后端

---

## 二、全周期路线图（P0 → P4）

> 原则：当前 P 全部做完且能演示，才进下一个 P。每个 P 内部按列出顺序做。

| 阶段 | 一句话目标 | 验收信号 |
|---|---|---|
| **P0** | 跑通核心链路（复刻 Handy 最小集） | 按 `Alt+Space` 说话，松手后正确文字出现在任意输入框 |
| **P1** | 变成 typeless（provider 抽象 + AI 润色 + BYOK） | 填自己的 key，说一段带口水话的话，输出是润色过的干净文本 |
| **P2** | 真流式上屏（差异化核心） | 感知延迟从松手后 2-5s 降到 ≈1s，且与说话时长无关 |
| **P3** | 体验打磨（词典、上下文 prompt、onboarding、fn 键） | 不看 README 5 分钟能跑通；vibe coding 场景下转写偏代码风格 |
| **P4** | 跨平台 + 分发 | Windows msi、macOS dmg 自动构建签名，updater 工作 |

P0 / P1 的详细 spec 见 §3 / §4。P2–P4 见 §5（高层目标 + 关键风险，进入时再细化）。

---

## 三、P0 · 详细 spec

### 3.1 切片清单（每个独立 commit，不收尾不开下一个）

| 切片 | 内容 | 验收 |
|---|---|---|
| P0.1 | 全局快捷键 → 空胶囊弹出/消失 | 按下 `Alt+Space` 胶囊出现，松开消失 |
| P0.2 | 按住录音 + 波形 | 胶囊里实时显示音量条 |
| P0.3 | VAD 断句（Silero, 抄 Handy） | 沉默 800ms 自动收尾 |
| P0.4 | 松手 → 调用 Groq `whisper-large-v3-turbo` → 控制台打印文字 | 控制台拿到正确文字 |
| P0.5 | 剪贴板法注入光标处（保存→写入→粘贴→恢复） | 任意输入框拿到文字 |
| P0.6 | 设置持久化（`tauri-plugin-store`） | 重启后快捷键 / 麦克风选择保留 |
| P0.7 | 错误态最朴素呈现（无权限 / 网络失败胶囊变红） | 三类错误能看到提示 |

### 3.2 核心数据流（hot path）

```
hotkey 按下
  → Rust 切到 RECORDING，发 state-change 事件
  → AudioManager 拉麦克风，连续发 audio-level 事件
  → VAD 监听静音

hotkey 松开（或 VAD 触发）
  → Rust 切到 PROCESSING，发 state-change
  → TranscriptionManager 把整段音频喂 AsrProvider
  → 拿到文字
  → InjectManager 走剪贴板法注入
  → Rust 切到 SUCCESS，发 state-change，150ms 后 IDLE

Esc 按下
  → Rust 切到 CANCEL，丢弃音频，发 state-change
  → 100ms 后 IDLE
```

**铁律**：前端不持有这条 pipeline 的任何状态，只镜像 `state-change` 事件渲染 overlay。所有业务在 Rust。

### 3.3 状态机

```
IDLE ──hotkey-down──▶ RECORDING ──hotkey-up/vad-silence──▶ PROCESSING ──ok──▶ SUCCESS ──150ms──▶ IDLE
                          │                                    │
                          └──Esc──▶ CANCEL ──100ms──▶ IDLE     └──err──▶ ERROR ──2.5s──▶ IDLE
```

非法转换（如 RECORDING 中再次按下 hotkey）→ 忽略，写 warn 日志。

### 3.4 平台抽象 trait（第一天就要有）

所有平台相关系统调用收敛到 `platform/mod.rs` 的一个 trait 后面。**不允许 `#[cfg(target_os)]` 散落 manager 里。**

```
trait Platform {
    register_hotkey(combo) -> HotkeyId
    unregister_hotkey(id)
    inject_text(text)              // 剪贴板法
    store_secret(key, value)       // P1 用，P0 留空实现
    read_secret(key)
}
```

P0 只实现 macOS（`platform/macos.rs`）。`windows.rs` 留空文件 + `unimplemented!()`，P4 再写。

### 3.5 Tauri Command 清单（前端 → 后端）

> 命名约定：`snake_case` 动词开头。返回值统一 `Result<T, AppError>`，具体类型 AI 开工时定。

| Command | 语义 | 关键字段 |
|---|---|---|
| `start_recording` | 手动触发录音（debug 用，正式走快捷键） | — |
| `stop_recording` | 手动停止 | — |
| `cancel_recording` | Esc 触发，丢弃音频 | — |
| `get_settings` | 读全部设置 | — |
| `update_settings` | 写设置（部分更新） | `patch: SettingsPatch` |
| `list_microphones` | 枚举可用麦克风 | — |
| `request_permission` | 申请系统权限（麦克风 / accessibility） | `kind` |
| `get_permission_status` | 查权限 | `kind` |

### 3.6 Event 清单（后端 → 前端）

| Event | 语义 | 关键字段 |
|---|---|---|
| `state-change` | 状态机切换 | `from`, `to`, `reason?` |
| `audio-level` | 实时音量（驱动波形） | `level` (0–1)，约 30 FPS |
| `partial-transcript` | P2 才有，实时字幕 | `text`, `is_final` |
| `final-transcript` | 整段转写完成 | `text`, `duration_ms` |
| `error` | 错误（见 §3.7） | `code`, `message`, `recoverable` |

> AI 实现时：所有 event 必须从 Rust 的 manager 发出，**不允许前端伪造 event 驱动自己**。

### 3.7 错误大类（具体错误码 P0 开工时定，但分类先定死）

| 大类 | 含义 | 用户看到什么 | 是否 recoverable |
|---|---|---|---|
| `Permission` | 麦克风 / accessibility 没授权 | 「请授予麦克风权限」+ 引导按钮 | 是（授权后重试） |
| `Device` | 麦克风设备问题 | 「找不到麦克风」 | 是 |
| `Network` | 转写 API 请求失败 / 超时 | 「网络失败，已撤销」 | 是 |
| `Provider` | API key 无效 / 配额耗尽 | 「key 无效，请检查」 | 否（要去设置改） |
| `Inject` | 剪贴板写入 / 粘贴失败 | 「注入失败，已复制到剪贴板」 | 部分（fallback 是放剪贴板） |
| `Internal` | bug，不该发生 | 「出错了，详情见日志」 | 否 |

错误一律不 panic，热路径**禁止 `unwrap()` / `expect()`**。

### 3.8 Overlay 状态规范

胶囊：屏幕底部居中、距底 16px、透明背景、点击穿透、永远置顶。数据全来自 Rust event，前端不持有业务逻辑。

| 状态 | 视觉 | 数据来源 | 过渡 |
|---|---|---|---|
| IDLE | 不显示 | — | — |
| RECORDING | 胶囊淡入。左：跳动波形；右：占位「录音中…」（P2 换成 partial 文字横向滚动） | `audio-level`（波形） | 上滑淡入 120ms |
| PROCESSING | 波形收成一条做呼吸脉冲，文字变暗 | `state-change` | 波形→脉冲 morph 200ms |
| SUCCESS | 一闪确认（细微高亮），随即淡出 | `state-change` | 注入完成后淡出 150ms |
| ERROR | 变红 + 短消息 | `error` | 抖动一下提示，停 2.5s 后淡出 |
| CANCEL | 用户按 Esc，直接淡出不注入 | `state-change` | 即时淡出 100ms |

**实现纪律**：别先画静态稿。直接在代码里搭空壳 + 几个按钮手动切状态，调动效手感。这部分动效手感是 P2/P3 才打磨的点；P0 阶段「录音中…/处理中…」文字占位即可。

### 3.9 P0 验收清单

完成 P0 时，AI 把这清单跑一遍，每一条都要能演示：

- [ ] 按 `Alt+Space` 胶囊出现，松开消失（P0.1+P0.2）
- [ ] 录音时胶囊里波形跟着说话音量动（P0.3）
- [ ] 沉默 800ms 自动收尾（不靠松手）（P0.4）
- [ ] 控制台能看到 Groq 返回的正确文字（P0.5）
- [ ] 在 VS Code、Chrome 地址栏、微信都能正确粘贴（P0.6）
- [ ] 重启 App 后快捷键和麦克风选择还在（P0.7）
- [ ] 拒掉麦克风权限 → 胶囊变红 + 看得到提示（P0.8）
- [ ] Esc 中断录音 → 不注入文字（P0.8）

---

## 四、P1 · 详细 spec

P0 全部跑通且 commit 干净后才开。

### 4.1 Provider 抽象

两个 trait，各自一组 adapter。新增 provider = 加一个 adapter 文件，不改其他代码。

```
trait AsrProvider {
    name() -> &str
    transcribe(audio_bytes, config) -> Result<TranscriptResult>
    // P2 时扩 transcribe_stream
}

trait LlmProvider {
    name() -> &str
    enhance(text, prompt, config) -> Result<String>
}
```

**P1 必带的 adapter**：

- AsrProvider：`Groq`（whisper-large-v3-turbo）、`OpenAI`（whisper-1）、`WhisperCpp`（本地）
- LlmProvider：`OpenAICompatible`（base_url + api_key + model 三字段，覆盖 OpenAI / DeepSeek / 硅基流动 / Ollama）

### 4.2 BYOK + 系统 keychain

- API key 存系统 keychain（macOS Keychain Services；P4 Windows Credential Manager）。
- 设置里 key 字段：写入即调 `store_secret`，读取走 `read_secret`，**绝不**写进 `tauri-plugin-store` 的明文 JSON。
- 配置导出（设置 → 导出 JSON）：敏感字段用占位符 `"<keychain>"` 替换。
- 配置导入：遇到占位符 → 提示用户重新填 key。

### 4.3 润色（typeless 核心价值）

- 默认 prompt（系统内置 + 可自定义）：
  - 去口水话（嗯、啊、那个）
  - 修明显口误
  - 加标点、换行排版
  - **不许**改原意、不许加内容、不许翻译
- 设置里能：开关、选 LLM provider、改 prompt、加自定义指令（追加在系统 prompt 后）。
- 转写失败 → 不润色，直接出转写原文。润色失败 → 出转写原文 + 提示「润色失败但已注入原文」。

### 4.4 P1 新增 command / event

| Command | 语义 |
|---|---|
| `set_secret` | 写 key 到 keychain（用 `key_id` 标识，如 `groq_api_key`） |
| `has_secret` | 查某个 key 是否已配置（不返回内容） |
| `delete_secret` | 删 key |
| `list_asr_providers` | 列可用 ASR provider 元信息 |
| `list_llm_providers` | 列可用 LLM provider 元信息 |
| `test_provider` | 用当前配置发一个测试请求 |
| `export_config` | 导出配置（敏感字段占位） |
| `import_config` | 导入配置 |

| Event | 语义 |
|---|---|
| `enhance-progress` | 润色阶段开始/完成（驱动 overlay 二段进度） |

### 4.5 P1 验收清单

- [ ] 设置里能填三家 provider 的 key 并测试通过
- [ ] 重启 App 后 key 还在（在 keychain 不在 store）
- [ ] 关掉润色 → 出转写原文；开润色 → 出去口水化的干净文本
- [ ] 自定义 prompt 生效
- [ ] 导出 JSON 不含 key 明文
- [ ] 切 ASR provider 不用重启
- [ ] 不联网时不爆栈，给「网络失败」错误

---

## 五、P2 / P3 / P4 · 概览

进入对应阶段时再回来把这一节展开。

### P2 — 真流式上屏（差异化核心）

**目标**：感知延迟 ≈ 1s，与说话时长无关。

**关键能力**：
- ASR provider 加 `transcribe_stream` 方法，走 WebSocket（候选：豆包流式 / 阿里 Paraformer-realtime / Deepgram）
- 录音边采集边上传 chunk
- `partial-transcript` 事件实时驱动 overlay
- 按下快捷键即预热连接（消除冷启动）
- 润色与转写并行：partial 提前喂 LLM，定稿时增量替换
- 「说错改口自动修正前文」：LLM 拿完整 context 增量工作

**关键风险**：
- 不同流式 provider 协议差异大，trait 抽象要扛得住
- partial 抖动 vs 流畅性的权衡
- 注入策略改变：流式期间不能真注入（输入框抖动），等 final 一次性注入

### P3 — 体验打磨

- 个人词典（术语注入 prompt + 高置信度自动纠正）
- 上下文感知 prompt（抄 Voxt App Branch：IDE 偏代码、聊天偏口语）—— **vibe coding 场景的杀招**
- 5 分钟 onboarding（默认内置免费引擎，不填 key 也能用）
- 权限引导（Microphone / Accessibility / Input Monitoring 顺畅授权流）
- `fn` 键支持（换 CGEventTap 方案，`tauri-plugin-macos-input-monitor`）
- Overlay 动效精修（这时候才该出设计稿）

### P4 — 跨平台 + 分发

- Windows：写 `platform/windows.rs`（`RegisterHotKey` / `SendInput` 或 UIA / Credential Manager）
- GitHub Actions 多平台构建（`tauri-action`），dmg / msi / deb / AppImage
- Tauri updater（minisign 签名，公钥进 `tauri.conf.json`，私钥进 GitHub Secrets）
- macOS notarization（真公开分发时再搞）
- Linux 标「PR welcome」，Wayland 坑让社区填

---

## 六、架构铁律

### 6.1 Manager 模式（复刻 Handy）

核心功能拆 manager，启动时初始化挂在 Tauri state 上。命名一律 `XxxManager`：

- `AudioManager` — 麦克风采集、音频流
- `ModelManager` — provider 配置、key 引用
- `TranscriptionManager` — 转写编排
- `EnhanceManager` — 润色（P1+）
- `InjectManager` — 文本注入

### 6.2 Command-Event 单向

- 前端 → 后端：Tauri command（命令）
- 后端 → 前端：event（状态/数据广播）
- **前端不持有业务状态**，只镜像 event 渲染。Zustand store 里出现「业务判断逻辑」就是错的。

### 6.3 平台抽象层（§3.4）

所有 `#[cfg(target_os)]` 收敛到 `platform/`，其他模块平台无关。

### 6.4 Provider 抽象（§4.1）

ASR / LLM 各自 trait + adapter 文件。加 provider 不改其他模块。

### 6.5 错误处理

- 应用层用 `anyhow::Result`，库层（`asr/`、`llm/`、`platform/`）用 `thiserror` 定义错误类型。
- 热路径**禁止 `unwrap()` / `expect()`**。
- 错误最后转成 §3.7 的 `AppError` 大类发到前端。

### 6.6 安全 / 隐私底线

- API key 一律走系统 keychain，**绝不**进配置文件明文。
- 不自建任何后端，音频和 key 只在用户设备和用户的 API 之间。
- 导出配置敏感字段占位。
- 录音音频默认**不落盘**（除非 debug 模式显式开）。

### 6.7 异步 & 阻塞

- async 走 tokio。
- 阻塞操作（whisper 本地推理、文件 IO）放 `tokio::task::spawn_blocking`，别堵事件循环。

---

## 七、技术栈与目录结构

### 技术栈（已定，不要改）

- **框架**：Tauri 2
- **前端**：React + TypeScript + Zustand
- **样式**：Tailwind + daisyUI（克制主题，圆角调小、色板收敛。P3 再决定要不要换 shadcn/ui）
- **包管理**：pnpm（前端）、cargo（Rust）
- **关键 plugin**：`tauri-plugin-global-shortcut`、`tauri-plugin-store`、`tauri-plugin-clipboard-manager`

### 目录结构

```
src/                              # 前端，平台无关
├── App.tsx
├── components/
│   ├── Overlay/                  # 录音胶囊
│   └── Settings/                 # 设置窗各 tab
├── hooks/
│   ├── useRecordingFlow.ts       # 监听 Rust event 驱动状态机镜像
│   └── useAudioLevels.ts
├── store/                        # Zustand（只放 UI 状态）
└── types/                        # Zod schema（外部输入校验）

src-tauri/src/
├── lib.rs                        # 入口，Tauri setup，manager 初始化（别动 main.rs）
├── main.rs                       # 只调 run()
├── managers/
│   ├── audio.rs
│   ├── model.rs
│   ├── transcription.rs
│   ├── enhance.rs                # P1+
│   └── inject.rs
├── platform/                     # 平台抽象层
│   ├── mod.rs                    # trait Platform
│   ├── macos.rs
│   └── windows.rs                # P4 才填，P0 留 unimplemented!()
├── asr/                          # ASR provider trait + adapters
│   ├── mod.rs
│   ├── groq.rs
│   ├── openai.rs
│   └── whisper_cpp.rs            # P1+
├── llm/                          # LLM provider trait + adapters
│   ├── mod.rs
│   └── openai_compatible.rs      # P1+
├── pipeline.rs                   # 录音→转写→润色→注入 编排
├── state.rs                      # 全局状态机
└── error.rs                      # AppError 定义
```

---

## 八、参考实现 & 引用方式

写系统层代码前**先去看 Handy 对应实现**，按它的模式来。三类参考各有其用：

| 项目 | 用途 | 怎么用 |
|---|---|---|
| **cjpais/Handy** | 主骨架，照着复刻 | clone 跑起来，读 `AGENTS.md` + `src-tauri/src/lib.rs` + `managers/`。写快捷键/VAD/注入时回去抄模式 |
| **hehehai/voxt** | 产品思路 | 只看 provider 抽象、词典、App Branch 上下文 prompt 怎么做。**Swift 代码不抄** |
| **OpenWhispr** | BYOK 设计 | 看本地 + 云端切换的 UI 思路 |
| **voquill/voquill** & **kstonekuan/tambourine-voice** | 同栈 Tauri 实现交叉验证 | 写 overlay / 流式 pipeline 时对照看 |

> Handy 是纯本地、无润色、无 BYOK。它给的是**系统层骨架**；润色 + 流式 + BYOK 是本项目要往上加的差异化，要自己写。

---

## 九、术语表

| 术语 | 含义 |
|---|---|
| **partial transcript** | 流式 ASR 在说话过程中返回的中间结果，会被后续修正 |
| **final transcript** | 流式 ASR 标记为定稿的结果 |
| **VAD** | Voice Activity Detection，沉默检测（用 Silero 模型） |
| **BYOK** | Bring Your Own Key，用户自带 API key |
| **overlay / 胶囊** | 屏幕底部居中的录音状态浮窗 |
| **润色 / enhance** | 用 LLM 把转写原文整理成干净文本 |
| **provider** | ASR / LLM 后端实现（如 Groq、OpenAI、本地 whisper.cpp） |
| **adapter** | 某个 provider 的具体实现文件 |
| **manager** | Rust 端的核心功能模块（AudioManager 等） |
| **command / event** | Tauri 的前后端通信原语，命令是前→后，事件是后→前 |
| **App Branch** | Voxt 的概念，按前台应用切换 prompt（如 IDE 用代码风格 prompt） |
| **fn 键方案** | 用 macOS CGEventTap 监听 fn 键作为触发键，需要 Input Monitoring 权限 |
| **剪贴板法注入** | 保存剪贴板 → 写入目标文本 → 模拟 Cmd+V → 恢复剪贴板。最兼容的注入方式 |
