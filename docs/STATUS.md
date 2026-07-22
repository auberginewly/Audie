# Audie · 进度计划书（做到哪了）

> 现在完成了什么的快照。规范在 [CLAUDE.md](../CLAUDE.md)，接口/架构在 [PROJECT_SPEC.md](../PROJECT_SPEC.md)，未来方向在 [ROADMAP.md](ROADMAP.md)。
> 更新于 2026-07-22。

---

## 一句话现状

**核心听写闭环已跑通、真机验证全链路通**：按快捷键 toggle 录音 → 豆包流式 ASR 出 final → 智能润色 → 注入到当前光标处。差异化（真流式 + 润色 + BYOK）已兑现。

P0–P3 主体完成 + release-v1 外围大部落地（onboarding/Home/History 已接真、触发键自定义、设备选择、README 产品展示）。剩下：终态 toast 四态验收、真分发（dmg+签名+公证）；P4 跨平台刚起步——Windows 平台层第一块已写但**未编译验证**。

## 能力清单（已可用）

- ✅ 快捷键 **toggle** 录音（单击开始、再击结束）+ 底部胶囊状态机
- ✅ **豆包流式 WebSocket ASR**，松手 `is_final` sentinel 确定性收尾出 final
- ✅ **智能润色**（OpenAICompatible `/chat/completions`），失败兜底注入原文
- ✅ **剪贴板法注入**到光标处；注入前激活回录音前的前台 App（不失焦）；不还原剪贴板做手动粘贴兜底
- ✅ **BYOK + 系统 keychain**（Voxt 风格 `SecItem*`；设置页打开只做 presence check，明文仅在用户点小眼睛时返回 WebView，Provider 操作在 Rust 按需读 key）
- ✅ **provider 抽象**：ASR **纯云端**（Groq/OpenAI/豆包流式 + GLM/通义/StepFun；本地 ASR 已移除）、LLM（OpenAICompatible，多供应商预设 + 本地 Ollama/LM Studio 免 key 自动检测 + 动态拉 `/models`）；**输出 sanitizer 剥思考/前言 + `/no_think` 关思考**（云端也受益）
- ✅ **平台抽象层**：macOS 系统调用收在 `Platform` trait 后
- ✅ **dev 稳定签名**：根治钥匙串/辅助功能反复授权（见 [SIGNING.md](SIGNING.md)）
- ✅ **macOS 首启授权修复**（未 commit，方案见 [macos-onboarding-permissions.md](specs/macos-onboarding-permissions.md)）：麦克风申请改为带有效 Objective-C completion block 的 `AVCaptureDevice.requestAccess`，并为 Hardened Runtime bundle 补音频输入 entitlement；辅助功能统一为 AX trust；输入监控统一为与 `CGEventTap` 对应的 CoreGraphics ListenEvent；onboarding 请求后轮询真实状态，被拒绝时进入系统设置，输入监控开启后提供重启。修正云端 ASR 麦克风隐私说明并移除已退役的本机听写说明。隔离 bundle 真机授权验收待本切片收尾。
- ✅ **设备选择 escape hatch**（`eca04c9`）：设置页列麦克风/手动选/重启保留；显式选优先于 P0.7 自动挑、选中不可用走 `Device` 错；auto 行显示实际解析设备名并去重；**实时电平预览**当场确认麦在收声
- ✅ **录音内存保险丝**（Issue #8 修复，方案见 [recording-duration-limit.md](specs/recording-duration-limit.md)）：无固定录音时长限制；单次 retained buffer 512 MiB 上限；豆包流式音频队列 bounded + 溢出回 batch；`LastTake` 用 `Arc<AudioData>` 避免复制整段音频并在终态 settle 后释放
- ✅ **keychain 读取边界收紧**（`7b6e06a`）：设置页打开只做 presence check，点小眼睛 Reveal 才读明文；测试连接/动态模型改由 Rust 按需读 key；dev/release 独立 keychain service（对应 [keychain-settings-read-boundary.md](specs/keychain-settings-read-boundary.md)）
- ✅ **ASR endpoint 设置贯穿**（未 commit，方案见 [asr-endpoint-settings.md](specs/asr-endpoint-settings.md)）：GLM / 通义 Fun-ASR / StepFun 的 endpoint 已贯穿 `settings.toml`、Rust `TranscriptionConfig`、provider adapter、前端 Zod 与配置弹窗；旧配置自动补官方默认值。同步修复豆包流式队列 overflow 转 batch 时丢失 endpoint/resource_id/key 的兼容缺口，避免误报「Doubao endpoint 未配置」。验证：Rust 187 通过、1 ignored，默认 target clippy、TypeScript、build、Prettier、schema 测试通过；lint 0 error / 40 个既有 warning。
- ✅ **Home 字数趋势收尾**（`bd300af` / `a116930`）：真实日聚合支持近 7/30/60 天；X 轴固定 7 个像素等距日期且折线首尾贴边；双线分别取主题亮紫 `--accent-text` / 深紫 `--accent-fill`；Recharts 固定 3.8.1 消除 7 天生长动画末帧 dash 切换；关闭图表焦点层，点击不再出现蓝框。README 已收敛为极简产品介绍并使用 Desktop「展示图2」（`9f13d26`）。验证：Node 图表测试 4/4、HistoryManager 14/14、`fmt` / `clippy -D warnings` / `typecheck` / `lint` / `build` 通过，React Doctor 对本切片文件零诊断；Playwright 实测三档请求、7 个等距日期、首尾路径、主题色、55 帧动画与点击焦点均符合验收。
- ✅ **模态弹窗 stroke 统一**（未 commit，方案见 [dialog-stroke-consistency.md](specs/dialog-stroke-consistency.md)）：设置、模型配置、首次配置向导和 History 清空确认统一使用 `1px gray-alpha-100` 外边框；菜单、下拉、toast 与录音胶囊保持不变。
- 🟡 **Windows 平台层第一块**（`9964919`，**未编译验证**）：hotkey（RegisterHotKey + 消息线程，主/写作双 slot）、剪贴板法注入、Credential Manager 存 key、系统语言；windows-sys 按 target 门控。本机 Homebrew Rust 无 rustup 装不了 windows-msvc target，cfg 门控下默认 target 不编译它——待 Windows 真机或 CI 多平台验证，验证前不算收尾切片
- ✅ 配置导入导出（敏感字段占位）、错误态分类、AirPods/Continuity 设备绕开

## 真实 / mock / 待办

| 状态 | 内容 |
|---|---|
| **真实（接了后端）** | 听写闭环、provider 选择 + BYOK、keychain、豆包流式、注入焦点/剪贴板兜底、快捷键、润色开关/prompt、provider test、dev 签名、设备选择 + 实时电平预览、**录音内存保险丝**（无固定时长限制、512 MiB retained 上限、豆包 bounded queue + overflow batch fallback、`LastTake` Arc/settle 释放）、**onboarding（权限×3 真实查询·申请 + 深链兜底、首启持久化自动弹、模型「已配置」实时 has_secret + ASR 门控、试一下录音注入、🌐 引导）**、**设置持久化（`settings.toml`，已移除 tauri-plugin-store）**、**模型测试连接**（豆包 WS 握手验鉴权 / groq·OpenAI 兼容探 /models，显示耗时 ms）、**Home/History（SQLite `HistoryManager`，`feat/history-home` `ed32df1`）**：Home 近 7/30/60 天真实统计与等距双线趋势；History 单条时间线、条目内原文/润色双框各自可复制、原文框可重试润色（重调当前 LLM 不用音频）、`history_retention` 留存清理、删除/清空；记录口径=成功存原文+润色·取消有转写则存·没识别到内容(静音/空 ASR)记「没有识别到内容」·硬错误不记；音频不落盘（§6.6） |
| **mock（忠实假数据，待接后端）** | 模型标签（自动更新流 mock UI 已整删、更新器延后 v1.1，见 release-v1；模型评分假数字已删，release-v1「没有假数据残留」一项推进） |
| **pending（已知待办）** | 终态 toast 四态（取消→撤销 / 润色失败→去设置 / inject 失败→插入原文·重试 / network→重试）**实现就位；作者 2026-07-18 拍板免除逐态走查**（前后端已通，日常在用无异常，出问题再开切片修）；§5.4 豆包「松手→注入」延迟数据正式落档；**`feat/provider-adaptation` 新增 ASR provider（GLM `asr/glm.rs` / 阿里云 `asr/aliyun/` / StepFun `asr/stepfun/`）= 离线编译 + 单测通过、真机未联网验证**：endpoint/字段/错误码映射 reverse-engineered from Voxt、未对照官方文档（GLM 非流式 `{"text":...}` 字段、`stream=false` 是否被接受、SSE 事件 `type` 名等均未验证，源码内 TODO 标注）。按 CLAUDE.md「每个里程碑独立可用、可演示」，真机验证前不可视为已收尾切片；验证通过后清各 TODO；完整 Windows 适配（权限实查、注入焦点、真机听写闭环）+ CI 多平台（平台层第一块已写未验证，见能力清单 🟡 行） |

---

## 附录 · 实现轨迹（P0–P3 切片，原样保留备查）

> 实现细节的流水档，从原 CLAUDE.md「当前进度」搬来。日常看上面的快照即可；要追某个切片怎么做的看这里 + git log。

**全周期**：P0 核心链路 ✅ · P1 typeless（provider+润色+BYOK+keychain）✅ · P2 真流式上屏 ✅ · P3 体验打磨（进行中）· P4 跨平台（起步：Windows 平台层第一块 `9964919`，未编译验证）

> **2026-07-18 收尾记**：内存保险丝（`76f9966`）与 keychain 读取边界（`7b6e06a`）两个切片，本文件早已记为完成，但**代码一直躺在工作区未 commit**——和 P1 整片被静默丢失同类险情，本次 session 复核 git status 时发现并落盘。教训复述：切片收尾以 `git log` 有记录为准，不以文档记载为准。

**P0 切片**
0. [x] 脚手架（Tauri 2 + React + TS + Tailwind v4 + zustand + zod；目录按 SPEC §7）
1. [x] 快捷键 → 空胶囊（主窗 + overlay 两 webview；`HotkeyRegistry` 解耦；状态机 IDLE↔RECORDING 由 Rust `state-change` 驱动）
2. [x] 按住录音 + 波形（cpal；AudioManager 双线程，emit ~30 FPS `audio-level`；`Info.plist` 加 `NSMicrophoneUsageDescription`）
3. [x] 松手转写打印（`AsrProvider` trait + Groq adapter；multipart + 手写 WAV；攒 f32 buffer，`stop_capture` 交整段 `AudioData`）
4. [x] 剪贴板法注入（`InjectManager` → `Platform::inject_text`；clipboard-manager 存写，core-graphics CGEvent 合成 Cmd+V）
5. [x] 设置持久化（`tauri-plugin-store`；`get_settings`/`update_settings`；快捷键从存档读、改键注销旧注册新）
6. [x] 错误态底座 + 麦克风权限红胶囊（`AppError` code/message/recoverable；`settle_to_idle` 统一出口；`ensure_microphone_permission` 走 TCC gate）
7. [x] 错误态收尾 + 蓝牙/Continuity 绕开（注入预检 `CGPreflightPostEventAccess`；Groq 错误分类；静音兜底；CoreAudio HAL transport 打分避开 AirPods/iPhone）

**P1 切片**
1. [x] Provider 配置骨架 + 列表（`asr_provider`/`llm_provider`/`enhance_*`；`list_*_providers` Voxt 风格 metadata；`update_settings` 用 patch）
2. [x] Keychain secret command（`set/has/delete_secret`；重启仍在；store 无明文）
3. [x] BYOK 设置页 + provider test（Groq/OpenAI/OpenAICompatible 三块 key；`test_provider` 探 `/models`）
4. [x] ASR provider 切换（Groq/OpenAI/WhisperCpp 骨架；切换不重启；缺 key/模型走 Provider/Device 错）
5. [x] LLM 润色链路（`llm::LlmProvider` + OpenAICompatible adapter；`EnhanceManager`；`enhance-progress` 事件；润色失败注入原文不进 Error）
6. [x] 配置导入导出 + P1 收尾（敏感字段 `"<keychain>"` 占位；导入提示重填）
- [x] 收尾修复：Keychain 反复弹窗（presence check + `SecItem*`）；Provider test Voxt snapshot 模型。2026-07-14 再修设置页回归：打开配置不再读明文；点小眼睛仍可显式 Reveal；测试连接/动态模型改由 Rust 按需读取；Debug/Release 使用独立 Keychain service，避免不同签名身份共享 ACL。详见 [keychain-settings-read-boundary.md](specs/keychain-settings-read-boundary.md)。

**P2 切片**
1. [x] P2 SPEC（§5 切片表 + `transcribe_stream`/`partial`/`final` 语义）
2. [x] 流式合同（`AudioChunk`/`TranscriptDelta`/`TranscriptStream` + `transcribe_stream` stub，不接热路径）
3. [x] 豆包二进制 codec + 单测（`asr/doubao/{mod,codec}`；gzip + 抽 `result.text`；14 单测；依赖 `flate2`）
4. [x] 豆包配置 UI + keychain（AppID/Token 走 keychain，endpoint/resource_id 进 store；`DoubaoSettings.tsx`）
5. [x] AudioManager PCM chunk 实时出口 + dev 连通性 command（cpal broadcast 一路；`asr/doubao/client.rs` ws；依赖 `tokio-tungstenite`）
6. [x] 录音热路径接流式（豆包配好则并行送 ws，松手用豆包 final 替批量；未配走 P1 批量）
7. [x] P2 收尾（退役 preview 开关；豆包配好即默认流式〔**P3 改为显式选用**，见下〕；不引 provider routing 新抽象）

**P3 切片（在 `fe` 分支「设计稿全还原」+ 胶囊全交互期间落地）**

> 切片编号统一用 SPEC 的 `P3.x`。旧的 `fe.8/fe.8a/fe.8b-1/fe.8b-2/fe.8c/fe.6` 临时编号已废弃，不再使用。

- [x] P3.1 Design System v1（Figma 组件库）
- [x] P3.2 Overlay 胶囊视觉重写（五态 + 7 条波形 + 自绘对勾 + toast；移除 daisyUI、overlay 透明）
- [x] P3.3 设置页 IA 重排（模型选择器 + `ModelConfigDialog` + 服务商/文本处理/通用/关于；Home/History/Wizard 为忠实 mock）
- [x] P3.4 设备选择 escape hatch（`eca04c9`）：`input_device` 设置（空串=自动）+ `list_microphones`（cpal 枚举，名字与 `resolve_input_device` 同源回环匹配）；显式选优先于 P0.7、选中不可用走 `Device` 错（不静默退默认）；auto 行显示实际解析设备名 + 列表去重；麦克风实时电平预览（AudioManager 独立 level-only monitor 通路 + `mic-monitor-level` 事件，`level_only` 不累积 buffer，录音启动时让出麦）
- [x] P3.5 Toggle 录音控制（`handle_hotkey` 只在 `Pressed`）
- [x] P3.6 Overlay 可交互控制模型（非激活 NSPanel 可点不抢焦点；✕/✓ + 终态 toast 动作；6 个 overlay→后端命令）
  - ⚠️ 终态 toast 四态（取消→撤销 / 润色失败→去设置 / inject 失败→插入原文·重试 / network→重试）**实现就位但未逐态真机验收**，挪到 pending。
- [x] 壳 + 注入焦点 + 流式收尾 + 签名（960×640 窗口 + 暗色 AppShell + 侧栏；注入前 `NSWorkspace.activate` 回前台 App；豆包 `is_final` sentinel 收尾；dev 稳定签名 + 注入剪贴板兜底；详见 [SIGNING.md](SIGNING.md)）
- [x] 豆包改显式选用（`f5c9244`）：豆包不再靠 keychain token 隐式劫持所有录音；改 `asr_provider = "doubao_stream"` 显式驱动，仅「选了豆包 + token 存在」走流式，选 Groq 真能切回批量；`doubao_stream` 在写校验 / 回读 / 前端 Zod 三处放行；豆包 badge 改 `has_secret` 真实判定。
- [x] P3.8 触发键输入监听探针（dev-only）：listen-only `CGEventTap`（platform 层）捕获 fn/单键/组合键，emit `trigger-probe-key`；IOKit `IOHIDRequestAccess` 主动弹 Input Monitoring 授权（缺权限走 `Permission` 引导）；`start/stop_trigger_probe` 仅 `#[cfg(debug_assertions)]`，不替换主触发键、不动状态机。**编译/clippy/67 单测全绿；真机走查（按 fn/F13/组合键 + 授权后重启）待手动验收。** 合同见 SPEC §5.8。
- [x] P3.9 自定义触发键正式接入（默认 fn）（M1 `418c09d` / M2 `855c15a` / M3 `d825186`）：把探针升级成生产主触发键，**统一一套 `CGEventTap`，删 `tauri-plugin-global-shortcut`**。`register_hotkey` 解析触发串（`parse_trigger`）成 fn / 单键 / 组合键，统一 tap 里匹配触发即调同一 callback（`handle_hotkey` 事件源无关，下游零改动）。fn = 干净「按一下」(down→up 无其他键 <400ms)；listen-only 不 swallow（组合键可能漏进前台 App，权衡见 `macos.rs` 注释）。默认键改 `Fn`；缺 Input Monitoring 注册不 abort 启动、走 `Permission` 引导。设置页 HotkeyRecorder 改 curated chip picker（fn/F13/F14/三组合键），加输入监控真实权限行（`get/request_input_monitoring_permission`）；hotkey 校验放宽（真 gate 在 parse）。`HotkeyEvent` press/release 遗留删除 → `Box<dyn Fn()>`。**编译/clippy/77 单测/tsc/build 全绿；真机走查（授权+关🌐→按 fn 录音注入、改键 F13/组合键即时生效、权限行真实状态、fn+方向键不误触发）待手动验收。** 合同见 SPEC §5.8。
- [x] P3.10 录制器原生化（`a2fb8bb`，作者系统调研 A–E 后定）：录制器整条**下沉到原生 CGEventTap**，前端不再用 WebView KeyboardEvent（看不到 fn + 被设置页 30fps 重渲染冲状态）。新增纯捕获状态机 `capture_step`（events→触发串，含裸修饰键时间窗去抖 / 组合 / fn+组合 / 单键）+ `name_for_keycode` 反向 + listen-only 捕获 tap（emit `trigger-captured` / `trigger-capture-rejected`）；Platform 加 `start/stop_trigger_capture`。**支持 fn+组合键**（`parse_trigger` 加 fn 作组合修饰 → "Fn+Space"=`Combo{Space,SecondaryFn}`，主触发检测无需改）。拒绝策略：Caps Lock + 系统破坏性组合（Cmd+Q/W/Tab/Space/H/M），媒体键天然抓不到。决策：**不做 active-consume 吞键**（listen-only，吞键以后单开切片）。前端 HotkeyRecorder 重写成 begin→listen→stop，删全部浏览器抓键。**编译/clippy/85 单测/tsc/build 全绿；真机走查（fn/单 Shift·Ctrl·Option·Cmd/F13/Ctrl+Shift+D/fn+Space 逐一录上并能触发；Caps Lock/Cmd+Q 被拒）待手动验收。**

- [x] P3.12 onboarding 真实化（`66c62ed` / `ea86216` / `379da76` / `bf0b22a` + 收尾；fe 分支，作者选「全做一个大 session」拆 5 commit）：SetupWizard 从忠实 mock 全量接真后端。**① 权限**：新 `usePermissions` hook + 4 个专用 command（镜像 P3.9 input_monitoring），麦克风 `check_microphone_permission`（不弹）/ 辅助功能 `CGPreflight·CGRequestPostEventAccess`；每行真实状态 +「授权」+「打开系统设置」深链兜底（修 opener capability 放行 `x-apple.systempreferences:` scheme，原被 `allow-default-urls` scope 拦）+ 加 Input Monitoring 行；focus 重读。**② 首启持久化**：`onboarding_completed` 设置（镜像 enhance_enabled 7 处 + `serde(default)`），首启自动弹/完成不再弹/中途关下次再弹。**③ key 校验**：退役 `ModelMeta.status` mock，新 `useConfiguredModels`（has_secret 实时算，wizard+设置页共用），ASR 步门控在 key 齐（豆包要 app_id+access_token 两个）。**④ 试一下**：textarea + 真按 fn 走真 hotkey 闭环，成败锚 `state-change(SUCCESS)`/`error` 事件非 textarea 内容。**⑤ 🌐 引导**（仅默认 fn 显示，深链键盘设置）+ 失败路径 permission 深链。**自动门禁全绿（clippy/85 单测/tsc/build/capability 编译）；真机走查（清 store 重启自动弹→三权限授权含深链→选 ASR 配 key→试一下注入→完成不再弹）待手动验收。**

> 近期候选切片、产品方向 → 见 [ROADMAP.md](ROADMAP.md)。
> 已知未做：fn 弹表情仍 best-effort + 引导关🌐（Tahoe 外接键盘连 tap 都收不到，要 DriverKit）；active-consume 吞键；Windows（`WH_KEYBOARD_LL`，逻辑层共用捕获状态机）。
- [x] P3.12 后续修复 + 持久化迁移（`38e71a1`…`acd6832`，fe）：onboarding 收尾后的打磨 + 一处架构迁移。
  - **onboarding/设置打磨**：关于页「重新运行配置向导」按钮；配置向导每步「完成即打勾」+ 进度条按步 `x/5`（「试一下」经 recording store `everSucceeded` 持久化，跨重开 wizard 仍在、app 重启重置）；豆包 badge 只认 access_token（app_id 后端可选）；模型「测试」接真——豆包新增生产命令 `test_doubao_connection`（开 WS + 发握手 + 读一帧验鉴权，不发音频）、groq/deepseek 走 `test_provider` 探 /models、成功显示耗时 ms、本地 Whisper 无测试按钮；LLM 两张卡当预设（选用仅在「仍是出厂默认/空」时填该家端点+模型；自定义过就只设 llm_provider、不覆盖——`acd6832`，从自定义切走改用「配置」弹窗）；测试结果纯文本无 icon。
  - **持久化迁移 TOML（Part 1/2/3）**：① LLM 选择从 `openai_compatible_base_url` 派生（不再本地 useState）；② 用户设置从 tauri-plugin-store(JSON) 迁到可手改的 `settings.toml`（`load/persist_settings` 整 struct serde + 新 `normalize_settings` 收编原读 helper 校验 + `app_config_dir` 路径 + `toml` 依赖）；③ **彻底移除 tauri-plugin-store**（Cargo/plugin/KEY_*/read helpers 全删），迁移改自包含 `migrate_from_legacy_json`（`serde_json` 直读旧 json，未知键忽略、缺字段 `serde(default)`）。key 仍在 keychain；前端无感（仍走 `get/update_settings`）。
  - **回归修复**（`62134e8`）：whisper 字段 serde `skip_serializing_if`（TOML 无 null）令 `get_settings` 省略该字段，前端 Zod `nullable`（要求存在）→ `safeParse` 失败 → `settings` 恒 null → 设置页「文本处理/通用」空白。改前端 `z.string().nullish()` 容忍缺失 + useSettings parse 失败补 `console.error`。
  - **门禁全绿**（fmt/clippy/89 单测/tsc/build）。真机待验：① 设置页四分区都在、配置都对；② 删 `settings.toml` 留 `settings.json` 重启 → 从旧 json 重新迁出、配置不丢；③ 模型「测试」豆包鉴权对/错两态 + groq/deepseek 探测。
- [x] 润色升级 + 主语言 + 润色区 UX 打磨（替代原 P3.11；方案见 `~/.claude/plans/a-app-prompt-inherited-wolf.md`）：
  - **P3.11 上下文 prompt（按 App 区分）作者拍板搁置**——不做 App Branch，本切片改做润色体验升级。
  - **默认润色 prompt = 数据文件、源码零 prompt**（作者反复强调不要硬编码）：内容（蒸馏自 Voxt 清理规则 + OpenWhispr 防注入开头，9 条优先级）放 `src-tauri/prompts/enhance_default.md`，`Settings::default().enhance_prompt` 用 `include_str!` 读进来——`.rs` 里没有任何 prompt 字符串、也没有命名 const。它只是「润色提示词」设置框的出厂默认值，用户改了存自己的 `settings.toml`。退役了 `DEFAULT_ENHANCE_PROMPT` / `LEGACY_ENHANCE_PROMPT` 两个 const + 旧默认升级逻辑。改写·写作的 prompt 以后单独补。
  - **去重**：`llm/mod.rs` 的 user message 改成只放转写原文（删掉 `请润色…只输出…` 祈使包裹，与 system prompt 重复且和反注入立场冲突）。润色指令只剩 system prompt 一处。
  - **主语言**：新增 `primary_language` 设置（空串=跟随系统，镜像 input_device）；`Platform::system_language()`（macOS 读 `NSLocale preferredLanguages` 首项→label，纯函数 `label_for_language_code` 带单测）；`enhance_config` 发送时把「用户主要语言：X」**前置一行**到 prompt（不是模板变量、框里看不到，用户只在下拉选；避免理解 `{{}}` 变量、也避免误译）。下拉只留**跟随系统 / 中文 / English**。
  - **润色区 UI**：润色提示词可折叠（默认收起+单行预览，展开编辑器）；「AI 润色说明」文案人性化 + 点明润色可选；「润色模型」行**直接显示在用模型名**（`openai_compatible_model`，不再写死 provider 名）+ 可点跳设置 模型 tab LLM 子页（`SettingsDialog` 提升 `modelType` + `goToModelLlm`，`ModelSection` type 受控）。
  - **`Select` 弹层修裁切**：portal 到 `<body>` + fixed 定位 + `max-h` 内部滚动，逃出设置面板 `overflow` 裁剪（主语言下拉曾被切断）；通用页麦克风下拉一并受益。
  - 删了遗留 `settings.json`（旧 tauri-plugin-store 文件，现 toml 在不再被读）。
  - **门禁全绿**（fmt/clippy 默认 target/90 单测/tsc/build）。真机待验：① 主语言下拉默认跟随系统、可切换持久化、弹层不被切；② 提示词折叠/展开/编辑；③ 润色模型显示模型名 + 点它跳 LLM 页；④ 配 LLM key 说一段含改口/口述标点/数字/中英混 → 注入符合 prompt 规则、语言不被统一。
  - 注：`cargo clippy --all-targets` 会翻出 `asr/doubao/codec.rs` 测试代码 2 处 `vec_init_then_push` 预存 nit（与本切片无关，未揽入）。
- [x] Provider 适配（`feat/provider-adaptation`，分块 4 commit `49ddab9`/`9524afe`/`a5c2787`/`0860305`；方案 `~/.claude/plans/provider-partitioned-shamir.md`）：① 云端 ASR 新增 GLM/通义 Paraformer/StepFun（仍真机未联网验证，见 pending 行）；② **本地 whisper.cpp 真推理**（whisper-rs 0.16 metal，用户自带 GGML 路径，每句重载、缓存/idle-unload 留待 P4）；③ `asr_model` 模型可选（groq/openai 等不再写死 const）；④ LLM 多供应商卡 + 官方端点预设 + **API key 小眼睛 Reveal**（点击才读 keychain，不点不弹）+ per-provider key/`llm_models` 模型存储；⑤ 模型选择器本地零配置（打开本地卡自动 fetch+选中）。
- [x] 本地 LLM/ASR 强化（方案同上 plan 的 P1–P3；workflow 实现 + 我接管修复）：
  - **P1 输出 sanitizer + `/no_think`**（`c8adb21`，手写）：剥 `<think>`/前言/代码围栏，仅剩推理则回退原始转写；system prompt 末尾追 `/no_think` 让 Qwen3 跳思考治延迟（其他模型忽略）；按 char 边界 case-insensitive 匹配，避免 `to_lowercase()` 字节错位 panic（含 ẞ/İ 回归测试）；6 单测。**provider 无关、云端也受益**。
  - **P2 本地自动发现**（`c421fa7`）：`discover_local_llm` 扫 Ollama 11434 / LM Studio 1234 / llama.cpp 8080，列在跑服务+模型一键选用（复用 `list_provider_models`/`is_chat_model`）。Voxt 用硬编码预设不扫端口——这是 Audie 差异点。
  - **P3 推荐模型按内存分档**（`2304ce2` + 修 `7f4abf6`）：8GB `qwen3:4b-instruct-2507` / 16GB `qwen3:8b` / 24GB `qwen3:14b` / 32GB+ `qwen3:30b-a3b-instruct-2507`（Q4_K_M；仅采信可验证型号，剔除网上虚构的 Qwen3.5/3.6/Gemma4；初版误把 30B-A3B 标 16GB+ 装不下，已修）。
  - **门禁全绿**（fmt/clippy `-D warnings`/全单测含 6 新 sanitizer/tsc/build）。真机待验：① 开 Ollama/LM Studio→「扫描本地」列服务+模型；② 选本地 Qwen3 录一句→输出无 `<think>`、干净注入；③ `/no_think` 经 Ollama OpenAI-compat 是否真省延迟（不省也不会污染，sanitizer 兜底；4B/30B-A3B 原生非思考稳）。
  - 教训：workflow 按片自提交时 **P1 整片被静默丢失**（panic 修复没过门禁→commit 被跳→改动回滚），靠主循环 `git log`+grep 复核才发现、改手写补回（记入 memory）。
  - **P4 模型下载器**（照 Handy 模式、只下 ASR `.bin`、本地 LLM 引导 `ollama pull`）+ whisper 上下文缓存/idle-unload + 模型校验/测试按钮：**后续独立项目，未开**。
- [x] 模型系统 v2 → **本地 ASR 整条移除**（净结果：ASR 纯云端）。先用 workflow 建了 v2（Phase 0–4：真 `ModelManager` + `models_catalog.toml` 目录 + 下载器〔流式/续传/sha256〕+ 开机扫盘 + 统一 install-state 选择器 + macOS 本机听写 `SFSpeechRecognizer`/objc2-speech）；其间靠**只读审计**抓回 5 处「能编译但 UI 没接通」同类回归（P1 整片被静默回滚丢失、本地 LLM 配置卡被删、macOS 本机听写不可达、onboarding 卡键less ASR）。随后作者拍板：本地 ASR 那套**难维护、整条砍掉**（`275e4a9`）——删 whisper.cpp 内置推理 + ModelManager/目录/下载器 + macOS 本机听写 + modelStore/本地卡/相关命令/设置字段/Speech 授权 + whisper-rs/objc2-speech/objc2-avf-audio/objc2-foundation/block2/sha2 依赖（7 文件）。**保留**：LLM 本地（Ollama/LM Studio **开机自动检测**〔无手动扫描按钮〕+ 配置卡 + 按内存推荐 Qwen3〔含 Gemma3/Granite4 备选〕+ 运行中模型合并进卡）、输出 sanitizer + `/no_think`、新增云端 ASR（GLM/通义/StepFun，仍真机未联网验）。顺带清掉 `model.rs`(843行,删除)/`ModelSection`(→357行) 两处超 400 行铁律债。门禁全绿（fmt/clippy `-D warnings`/165 测试/tsc/build）。
  - **后果（记一笔）**：ASR 不再有离线/纯本地选项，每次转写走云端 API（BYOK）；润色仍可本地。要找回离线 ASR 见 `275e4a9` 前的 git 历史。
- [x] 写作模式 compose（`c00b77d`，文本处理三模式补齐**片 1/2**；方案 `~/.claude/plans/happy-discovering-candy.md`）：润色之外新增「写作」——按独立写作触发键说要点 → LLM 以「生成」立场（区别润色的忠实清理；prompt 数据文件 `prompts/compose_default.md`，源码零 prompt）成稿 → 注入光标处，复用 enhance 链路 + 输出 sanitizer + `/no_think`。`DictationMode{Polish,Compose}` 贯穿 pipeline（录音开始按触发键 role 定 mode）；平台 trait 加 `HotkeySlot`，macOS 主键 / 写作键各自独立 CGEventTap slot，`apply_hotkeys_if_changed` 幂等复活另一 slot（修录制器 `begin_trigger_capture` 停所有键后、录一个键会让另一键失活）；设置加 `compose_hotkey/compose_enabled/compose_prompt`，TextSection 写作卡接真（开关 / 写作键录制复用 HotkeyRecorder / 提示词折叠）。门禁全绿（fmt/clippy `-D warnings`/test/tsc/build）。**真机待验收**：设置 → 文本处理 → 写作 tab 开启 + 录写作键 + 配 LLM key → 按写作键说「写封请假邮件」→ 光标处出干净生成文本（纯文本无 `<think>`/前后缀）；录写作键后主 fn 键仍可用（双 slot 复活）。
  - **改写 rewrite = 片 2/2，已做**（`dd2ad36`）：`DictationMode` 加 `Rewrite`；主键（fn）按下先探选中态——有选中→改写、无选中→润色（自动分流）。选中态检测走剪贴板 sentinel 法（写哨兵串→合成 Cmd+C→剪贴板变了=有选中，没变则还原剪贴板；避开新增 objc2 依赖）；选区存进 `RewriteSourceSlot`，lib 层把「原文+口述指令」拼成一个 user text 喂现有 `enhance()`（不动 LlmProvider 抽象）；rewrite prompt 数据文件 `prompts/rewrite_default.md`（「执行指令」立场，与润色反注入相反）；结果复用 `inject_text`（Cmd+V 落在仍选中的文字上即替换，零新增注入代码），失败兜底注入原文。改写历史的「原文」记录口述指令 + 引用的选中内容（`rewrite_history_raw`，`7c6b391`；finish_pipeline_tail 在 rewrite_text 消费前 peek 选区），看得出对哪段下了什么指令。**真机待验收**（重点）：多 App（VS Code/微信/浏览器/Notion）选中态不误判、大段选区 Cmd+C 120ms 时序、剪贴板还原。
  - **文本处理 IA 统一**：删掉 per-mode 启用开关——润色=配了 LLM 即生效、写作=配了写作键即生效、改写=选中即生效；三卡（润色/改写/写作）统一成「模型行（共享同一个 LLM）+ 提示词折叠 + 说明折叠」；写作触发键挪「通用」，触发键改名「润色/改写触发键」+「写作触发键」，两键不能相同（前端 `HotkeyRecorder.conflictWith` 实时拒 + 后端 `validate_settings` 兜底）。
  - **History 按 mode 区分 + Home 统计拆分**（`c21205f`/`6c35659`/`70cc254`）：`HistoryManager` 加 `mode` 列（polish/rewrite/compose，老库 `ALTER TABLE` 幂等迁移）；HistoryScreen 恢复「全部/润色/改写/写作」筛选 pill + 每条带 mode 标签；`UsageStats` 按 mode 分组——口述时间/字数/速度只算 `mode='polish'` 的 **ASR 转写原文**（`LENGTH(raw_text)`，不含润色增减），写作/改写产出单列「AI 产出·字」（删了原「节省时间」卡）。
  - **润色开关重加**（`e86203f`）：反转 f87b551 的「删开关」——重新给「AI 润色」开关，让人即使配了 LLM 也能选纯转写原文。默认 true（不回退旧默认行为）；`enhance_config_from_settings` 加 `polish_toggle` 参数，启用 = `force_enabled || (polish_toggle && 配了 LLM)`，compose/rewrite/重试走 force 不受开关管；开关关且非 force 时跳过 keychain 读取。门禁全绿（fmt/clippy `-D warnings`/test 167/tsc/build）。
  - 注：以上属 ROADMAP 的 **v1.x「输出质量」线**，作者拍板插到 release-v1 发布主线之前做。

> ⚠️ 切片编号口径：本附录把 `a2fb8bb` 记为 P3.10（录制器原生化，临时插入片），而 SPEC §5.5 的 P3.10=词典；onboarding 按 SPEC 排号记 P3.12。SPEC §5.5 P3.11=上下文 prompt 已**搁置**（作者不做按 App 区分）。词典仍未做，编号待作者统一。
