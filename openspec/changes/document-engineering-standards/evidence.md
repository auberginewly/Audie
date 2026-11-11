# 文档规范补全记录

基线 main 32af050，作者已确认“帮我补一下”。本片仅补公共工程文档并修正本机指引，没有修改运行时、依赖、CI、hook 或测试实现。

## 交付

- CONTRIBUTING.md：公共开发环境、运行/检查、切片与提交流程，区分前端预览和原生能力，说明本地尚无自动安装的 hook。
- docs/ENGINEERING.md：现有命名、400 行/测试就近、中文注释、接口、错误日志、线程/状态归属、前端资源释放与验收矩阵。
- README 双语入口、ARCHITECTURE 依赖边界与 STATUS 已接入；OpenSpec context 有公共阅读入口，不要求 clone 自带维护者本机草案。
- 本机 AGENTS.md / CLAUDE.md 指向公共细则，并对齐已有 Manager、TOML/SQLite、同步 provider/工作线程与实际 CI；二者保持忽略，未公开个人协作文件。只明确加入新 ENGINEERING 文档，保留整个 docs/ 的原忽略策略。

## 验证

- Prettier 明确检查所有本片 Markdown/YAML 及本机 AI 指引，通过；docs/ 命中 Git 忽略规则，检查时以 `--ignore-path /dev/null` 显式避免漏检，贡献指南注明该示例为 macOS 命令。
- 公共新指南、README、架构导航与本片 Markdown 的相对链接/锚点逐个检查；目标存在且在 Git 暂存索引中可见。STATUS 旧历史链接不在本片全量修复范围；新入口不依赖私有文件。
- CONTRIBUTING 的 pnpm 脚本逐个对照 package.json；Node 24、pnpm 11、Rust stable 及基本检查命令对照 CI。原生构建命令沿用现有 Tauri CLI；Tauri 官方前置条件已核对 macOS 桌面可使用 Command Line Tools。
- settings.toml、实际 ASR/LLM 同步签名、现有错误分类与模块依赖均回读当前实现，无按文档反向更改代码。
- `openspec validate document-engineering-standards --strict --no-interactive` 通过，skip_specs，不生成虚构能力合同。
- `git diff --check HEAD` 和暂存 diff 空白检查通过；最终路径限定为 Markdown 与 openspec/config.yaml 文档配置，既有业务、依赖、CI、测试源码无差异。

文档切片没有运行 Cargo/Node 业务测试、原生构建或真实系统演示，不将以前的结果写成本轮结果；其他 change 的待验收状态保持。
