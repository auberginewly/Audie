# Design

## Existing approved architecture

沿用 [总体设计](../refactor-module-organization/design.md) 的 S7 目标，无新依赖、状态层或 IPC 框架。基线为 `5c38a85e3d9ee68c8a150db292d7082c9d58b591`，实现工作区为 recovery-without-templates/Audie，分支 `codex/reorganize-modules`。原始测试与有效文案快照见 evidence/。

## Ownership and migration

完整旧→新映射见 [migration-map.json](evidence/migration-map.json)。原 hooks/types/store 与 Settings/screens/Overlay 目录不留兼容转发；原未使用的聚合入口移除。公共无业务组件留 components/ui，应用导航留 components/shell。DevicePicker 是设备业务 UI，移入 audio；共享 Select 不动。

- dictation：录音状态镜像、Zod 事件、录音订阅、音量环、Capsule。Waveform 维护可调波形；CapsuleToast 维护终态按钮与错误动作；overlayCommands 仅移动原有最佳努力调用函数供两者复用，没有扩大为通用 IPC 层。
- history：数据 schema / 查询、HistoryScreen、HistoryRow（条目版本、改写引用与动作）；home 单向读取 history 统计类型。
- providers：模型目录、配置、列表/发现/测试、secret 元数据、合同；text 只保留三份提示词的原编辑能力。
- hotkeys / audio / permissions：分别收拢录键、设备预览和权限状态/轮询；预览事件归 audio，仍用独立 mic-monitor-level，不能驱动听写波形。
- settings：SettingsSchema、加载/更新、设置窗口与通用/关于。完整设置 schema 使用 provider 自有 ASR/LLM enum；provider request 不再反向引用 SettingsSchema.shape，避免运行时循环。useSettings 仍同时启动原五个 Promise.all 请求、同批解析与更新，失败语义不变。
- onboarding：向导步骤、进度、SetupGuideCard、useOnboarding。App 创建唯一设置/权限/模型 hook 实例，再交给向导协调；没有新增轮询、设备监听或重复数据实例。
- i18n：入口、语言元数据、三种语言文件。zh-Hant 保留对 zh-Hans 的 fallback spread；默认 zh-Hans 与插值函数原样保留。每种语言不足 400 行，无需为行数机械拆 key 分组。

纯合同可以 type-only 引用完整 Settings，功能不引用 SettingsDialog、SetupWizard 或 App。TextSection 继续复用现有 SettingRow 展示，避免重复原始布局。

## Testing and tooling

现有六个前端测试文件移至功能 / shell 的 __tests__；release-version 留根 tests。run-tests.mjs 用 Node 遍历默认 tests + src，使用 process.execPath 调用原 Node runner；打印文件数量，零发现、测试失败、spawn 失败均非零退出。定向脚本只传目标功能目录。

Node mjs 单独使用 Node 全局作用域 lint，TypeScript 严格检查不放宽；Prettier 扩展 mjs。模型目录源码测试排除 __tests__，避免测试本身的 Groq 断言成为应用实现命中。没有增加框架、依赖或生产测试导入。

## Verification and limitations

证据和检查状态见 [evidence.md](evidence.md)。浏览器验证使用已安装 Chrome、production preview 与模拟 Tauri IPC；这验证真实打包 UI 的加载/交互，不证明 OS 权限、快捷键、焦点、注入、真实网络或钥匙串。原生集成验收由协调者记录，未验证场景不会标为通过。
