> ⚠️ **此文档已归档（2026-06-16）。**
> 当前 source of truth 是 [PROJECT_SPEC.md](PROJECT_SPEC.md)。
> 这份只保留作为思考过程留档，不再维护，**不要再以此为准**。

---

# 语音输入工具 · 项目规划

typeless 开源平替 · Tauri 2 + React + Rust · 主骨架复刻 Handy · 先 macOS 后跨平台

---

## 〇、参考策略（定稿）

| 项目 | star | 栈 | 怎么用 |
|---|---|---|---|
| **Handy** | ~2.3w | Tauri2 + Rust + React | **主骨架，照着复刻。** 同栈、明确为 fork 设计、系统层全成熟。先 clone 跑通读懂它的 AGENTS.md |
| Voxt | ~0.6k | Swift | 只看产品思路：provider 抽象、词典、App Branch、错误态。**不抄代码** |
| OpenWhispr | — | Electron + React | 看 BYOK（本地+云端）设计思路，系统层代码用不上 |
| Voquill / Tambourine | — | Tauri | 同目标 Tauri 实现，交叉验证 overlay / 流式 pipeline |

> Handy 是纯本地、无 LLM 润色、无云端 BYOK。它给你的是**系统层骨架**；润色 + 流式 + BYOK 是你要往上加的差异化。

---

## 一、架构（复刻 Handy 的核心模式）

### Manager 模式
核心功能拆成几个 manager，启动时初始化，挂在 Tauri state 上：
- **AudioManager** — 麦克风采集、音频流
- **ModelManager** — provider 配置、key 管理（你的扩展：ASR/LLM 两类）
- **TranscriptionManager** — 转写编排
- （你新增）**EnhanceManager** — LLM 润色
- （你新增）**InjectManager** — 文本注入（剪贴板法）

### Command-Event 架构
- 前端 → 后端：Tauri command（开始录音、保存设置）
- 后端 → 前端：event（状态变化、partial transcript、音量）
- 前端是状态机的镜子，不持有业务逻辑

### Pipeline
```
音频 → VAD(Silero) → ASR → [润色] → 文本 → 剪贴板/粘贴注入
```

### 状态流
```
Zustand → Tauri Command → Rust State → tauri-plugin-store 持久化
```

### 目录结构
```
src/                          # 前端（平台无关）
├── App.tsx
├── components/
│   ├── Overlay/              # 录音悬浮窗
│   └── Settings/            # 设置窗各 tab
├── hooks/
│   ├── useRecordingFlow.ts   # 监听 Rust event 驱动状态机
│   └── useAudioLevels.ts
├── store/                    # Zustand
└── types/                    # Zod schema

src-tauri/src/
├── lib.rs                    # 入口，Tauri setup，manager 初始化（别动 main.rs）
├── managers/
│   ├── audio.rs
│   ├── model.rs
│   ├── transcription.rs
│   ├── enhance.rs            # 新增：润色
│   └── inject.rs             # 新增：注入
├── platform/                 # ★ 平台抽象层（Handy 没显式分，你要加，为跨平台）
│   ├── mod.rs                # trait Platform
│   ├── macos.rs
│   └── windows.rs            # 后补
├── asr/                      # ASR provider trait + adapters
├── llm/                      # LLM provider trait + adapters
├── pipeline.rs
└── state.rs
```
> 比 Handy 多两样：`platform/` 抽象层（为跨平台）和 `asr/`+`llm/` 的 provider 抽象（为 BYOK + 流式）。这两个是你区别于 Handy 的地方，也是简历上能讲的设计。

---

## 二、UI（三个面，克制、功能性优先）

### UI 组件库决策：daisyUI

- **P0/P1 用 daisyUI 快速糊骨架**，不在 UI 上纠结。选一个克制的内置主题（`nord` 或自定义把圆角调小、色板收敛），别用默认那种圆润活泼的味道。
- 早期 UI 是消耗品，不值得精雕。**真正打磨视觉放到 P3**（onboarding + overlay 体验），到时再决定 daisyUI 够不够、要不要换 shadcn/ui（社区主流、无框架味、完全可定制，OpenWhispr 用的就是它）。
- 取舍认知：daisyUI 上手快写得少，但有"daisyUI 味"容易撞脸，和你要的克制留白审美略冲。用它就是图早期速度，别指望它出彩。

### 三个面

1. **菜单栏图标** — 常驻，点开：开始/停止、设置、退出。不进 Dock（可选）、开机自启（可选）。单实例：再次启动只把设置窗提前。**不需要设计方案**，标准托盘菜单。
2. **设置窗** — Model（ASR/LLM 分开配）、Shortcuts（tap / long-press）、Dictionary、General（麦克风、悬浮窗位置、自启、代理）。**不需要设计方案**，daisyUI 的 `tabs`+`input`+`toggle`+`select` 直接拼，布局照 Handy 设置页抄。
3. **录音悬浮窗 Overlay** — ★ **唯一需要提前想清楚的界面**。但要想的不是"长什么样"，是"状态怎么变"。它的本质是状态机 + 动效，不是静态布局。spec 见下。

### Overlay 状态机 spec（唯一要先定的 UI）

数据全部来自 Rust 的 event，前端只渲染、不持有逻辑。屏幕底部居中胶囊，透明背景、点击穿透、永远置顶。

| 状态 | 视觉 | 数据来源 | 过渡 |
|---|---|---|---|
| **IDLE** | 不显示（或极小一个点） | — | — |
| **RECORDING** | 胶囊淡入。左：跳动波形；右：实时 partial 文字横向滚动 | `audio-level` event（波形）+ `partial-transcript` event（文字） | 从底部 16px 上滑淡入，120ms |
| **PROCESSING** | 波形收成一条做"呼吸"脉冲，文字变暗/定格 | `state-change` event | 波形→脉冲 morph，200ms |
| **SUCCESS** | 一闪确认（细微高亮），随即淡出 | `state-change` event | 注入完成后淡出，150ms |
| **ERROR** | 变红，显示短消息（麦克风没权限/网络失败），停留稍久 | `error` event 带 code | 抖动一下提示，停 2-3s 后淡出 |
| **CANCEL** | 用户按 Esc，直接淡出不注入 | `cancel` 触发 | 即时淡出 |

**关键提醒**：这部分别先画静态稿——直接在代码里搭一个空壳，给几个按钮手动切状态，把动效手感和摆位调出来。波形怎么动、partial 怎么滚、淡入淡出的曲线，这些静态图看不出好坏，必须跑起来看。这也是你最在意的"UI 细节手感"能发力的地方，但它属于 P2/P3 的打磨，P0 阶段先用最朴素的"录音中…/处理中…"文字占位即可。

---

## 三、P0–Pn 功能排期

> 原则：P0 必须先全做完且能演示，再碰 P1。每个 P 内部按列出顺序做。typeless 的功能拆进各 P。

### P0 — 跑通核心链路（= 复刻 Handy 最小集）「立住项目」
没有这些就不是一个能用的工具。对标 Handy 的基础能力。

- **P0.1** 全局快捷键（官方 `tauri-plugin-global-shortcut` + 普通组合键如 Alt+Space，先不碰 fn）
- **P0.2** 悬浮窗弹出/消失（透明、置顶、点击穿透）
- **P0.3** 按住录音 + 波形（AudioManager）
- **P0.4** VAD 断句（Silero，抄 Handy）
- **P0.5** 批量转写（Groq whisper-large-v3-turbo，快且简单）
- **P0.6** 剪贴板法注入光标处（保存剪贴板→写入→粘贴→恢复）
- **P0.7** 设置持久化（tauri-plugin-store）

✅ P0 验收：按 Alt+Space 说话，松手后正确文字出现在任意输入框。**项目立住。**

### P1 — 变成 typeless（provider 抽象 + AI 润色 + BYOK）
这一层让它从「听写工具」变成「typeless 平替」。

- **P1.1** 抽出 `AsrProvider` trait，加 2-3 个 adapter（Groq / OpenAI / 本地 whisper.cpp）
- **P1.2** 抽出 `LlmProvider` trait（OpenAI-compatible：base_url+key+model）
- **P1.3** **AI 润色**（typeless 核心）：去口水话、修口误、加标点排版。预置 prompt + 可自定义
- **P1.4** BYOK 设置界面 + key 存 keychain（macOS Keychain Services）
- **P1.5** 配置导出/导入（敏感字段占位符替换，抄 Voxt）

✅ P1 验收：填自己的 key，说一段带口水话的话，输出是润色过的干净文本。

### P2 — 真流式上屏（★ 差异化核心，Handy/Voxt 都没有）
最有技术含量、最值得写简历和做视频的部分。typeless 快的本质在这。

- **P2.1** ASR 换流式 WebSocket（豆包流式 / 阿里 Paraformer-realtime / Deepgram）
- **P2.2** 音频 chunk 持续上传，partial transcript 实时回传
- **P2.3** Overlay 实时显示 partial（边说边出字）
- **P2.4** 连接预热（按下快捷键即建连，消除冷启动）
- **P2.5** 润色与转写并行（partial 提前喂 LLM）
- **P2.6** 「说错改口自动修正前文」（润色模型拿完整上下文增量工作）

✅ P2 验收：感知延迟从松手后 2-5 秒降到约 1 秒，且与说话时长无关。

### P3 — 体验打磨（typeless 的「懂你」那部分）
- **P3.1** 个人词典（术语注入 prompt + 高置信度自动纠正）
- **P3.2** 上下文感知 prompt（抄 Voxt App Branch：IDE 偏代码、聊天偏口语）—— **你的 vibe coding 场景在这发力**
- **P3.3** 5 分钟 onboarding（默认内置免费引擎，不填 key 也能用）—— 对 Voxt「README 长到要喂 AI」的降维打击
- **P3.4** 权限引导（Microphone / Accessibility / Input Monitoring 顺畅授权）
- **P3.5** fn 键支持（换 CGEventTap 方案，`tauri-plugin-macos-input-monitor`）

### P4 — 跨平台 + 分发
- **P4.1** Windows：重写 platform/windows.rs（RegisterHotKey、SendInput/UIA、Credential Manager）
- **P4.2** GitHub Actions 多平台构建（tauri-action）+ 自动更新
- **P4.3** Linux 标 "PR welcome"，社区填 Wayland 坑

---

## 四、施工顺序（对抗烂尾）

把 **P0.1–P0.6** 当成六个当天能做完的切片，每个独立 commit：

1. 按 Alt+Space → 空胶囊弹出/消失（系统层第一关）
2. + 按住真录音，overlay 显示波形
3. + VAD 断句
4. + 松手发 Groq，结果打印控制台
5. + 剪贴板法注入光标
6. + 设置持久化

六个做完 = P0 完成 = 一个能用的工具。**铁律：当前切片不收尾（能跑 + 干净 commit），不开下一个。**

---

## 五、动手第一步

1. `git clone` Handy，`bun install && bun tauri dev` 跑起来
2. 读它的 `AGENTS.md` + `src-tauri/src/lib.rs` + managers，理解 Manager 模式和 Command-Event 怎么串
3. 看 `capabilities/default.json` 理解 Tauri 权限怎么配
4. 确认 LICENSE（MIT，可基于它改，开源时按 MIT 标注保留版权）
5. 用 create-tauri-app 起自己的空项目，把 CLAUDE.md 放进去，开始 P0.1

> 别直接在 Handy 仓库里改——起干净的新项目，照着它的模式写，遇到系统层难点回去抄对应实现。这样代码是你的，你才讲得清每个决策。

---

## 六、如何 vibe 这个项目（工作流）

你前两次 vibe 翻车不是因为 vibe 本身错，是因为让 AI 替你做了架构决策、又一次铺太大。这次的 vibe 是"在已定地图上施工"，不是"让 AI 带你探险"。具体怎么做：

### 每个 session 的固定开场
1. 让 Claude Code 先读 `CLAUDE.md` 和里面的「当前进度」，明确这次只做哪一个切片。
2. 一次只做一个切片（P0.1–P0.6 那种粒度），一个 session 尽量对应一个 commit。

### 写代码前：先计划，你拍板
3. 用 plan mode / 先让它出方案——改哪些文件、为什么、和现有结构关系。**你看懂并同意了才让它写。** 它要引入新库或新抽象，停下来问你。这一步是防"AI 替你做架构决策"的闸。

### 写系统层时：先读 Handy 再写
4. 全局快捷键、文本注入、VAD 这种，让它**先去看 Handy 对应实现**再动手，别凭空生成。明确告诉它"参考 Handy 的 X，按它的模式来"。

### 写的过程：小步、看懂、能解释
5. 小 diff，每个 diff 你都过一遍。**看不懂、讲不清的代码不准合进去**——这正是 Competify 的教训（AI 生成的架构你解释不了）。宁可让它拆小、解释清楚。
6. 判断"做完"的信号是能跑 + 能演示，不是"AI 说写好了"。每个切片跑起来验证。

### 卡住时：回滚，别让它越补越乱
7. AI 开始"修 bug 引入新 bug、补丁摞补丁"时，**停**。`git reset` 回到上一个干净 commit，把切片重新拆更小再来。不要顺着它的螺旋往下走——这是 vibe 失控的典型死法。
8. 频繁 commit，git 就是你的 undo。每个切片收尾：跑通 → commit（message 写清为什么）→ 更新 CLAUDE.md 的「当前进度」。

### 用你已有的护栏
9. 你本来就在用 `.claude/settings.json` 配权限、CC Switch 管多 provider——继续用。给危险操作设确认，避免它自作主张大改。

### 一句话总结
**你负责架构和取舍，AI 负责在你画好的框里加速搬砖。** 你能对每个设计决策讲出 why，这个项目才是你的、才面试讲得清。这是它和"fork 个大工程改改"的本质区别。
