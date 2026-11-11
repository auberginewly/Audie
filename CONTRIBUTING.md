# 参与 Audie 开发

Audie 是 BYOK 语音输入桌面客户端：按快捷键开始说话，再次按键或点击确认结束，文字写入当前光标处。开发优先 macOS；Windows 改动注明实际验证范围。

## 先了解项目

- [工程规范](docs/ENGINEERING.md)：命名、注释、接口、错误日志与测试要求。
- [代码导航与依赖边界](docs/ARCHITECTURE.md)：功能归属和听写处理路径。
- [当前进度](docs/STATUS.md)：已实现能力、已知限制与待测项目。
- [变更方案](openspec/changes/)：每次切片的目标、设计和验证记录。

产品保持客户端直连，不自建后端，不引入账号/云同步/计费；会议转录、移动端、实时翻译和语音执行 agent 不在当前范围。功能和架构取舍由维护者确认，先说明要解决的问题与最小方案。

## 开发环境

- Node.js 24、pnpm 11（与当前 CI 主版本一致；确切版本以工作流为准）。
- Rust stable，安装 `rustfmt` 和 `clippy`。
- macOS 安装 Xcode Command Line Tools；其余系统依赖参考 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)。

在仓库根目录执行：

```sh
pnpm install --frozen-lockfile
pnpm tauri dev
```

首次启动需要按实际功能授予麦克风、输入监控和辅助功能权限。真实听写需要在设置中配置自己的 ASR，以及按需配置 LLM；API key 只保存在系统凭据存储，不写入仓库或示例配置。

`pnpm dev` 只启动前端，不提供 Tauri command、真实录音或系统注入。浏览器调试需要明确的模拟 IPC，不能据此宣称原生链路通过。

开发签名及权限排查见 [SIGNING.md](docs/SIGNING.md)。没有维护者的签名身份也可开发，不要修改或提交自己的证书配置。

## 一次改动的流程

1. 先读相关实现及测试，说明目标、非目标、受影响文件与验收动作。涉及快捷键、注入等系统功能时，先查看 Handy 对应实现并记录参考。
2. 有功能或架构取舍时先讨论；已确认的方案直接在约定范围内实施。不要借重构增加库、层或产品能力。
3. 在 `openspec/changes/<topic>/` 记录方案和任务。纯文档/内部整理可使用 `skip_specs: true`，不虚构产品规格；涉及行为时写清可观察场景。
4. 提交小而完整的切片，保留无关工作区修改，完成与改动相称的检查及真实演示。
5. 在变更记录中写下实际结果与未测项，更新 STATUS。未完成系统验收的 change 保持开放。

OpenSpec CLI 是文档工作流工具，不是运行应用的依赖。维护变更记录时可使用已安装的 CLI；该 CLI 不随 `pnpm install` 自动安装。

## 常用检查

以下命令均在仓库根目录运行；按 [验收矩阵](docs/ENGINEERING.md#测试与验收) 选择需要的检查。

```sh
# 前端
pnpm test
pnpm typecheck
pnpm lint
pnpm format:check
pnpm build

# Rust
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# 原生集成：包含前端构建，不生成安装包
pnpm tauri build --debug --no-bundle
```

涉及 Rust 测试或共享测试 helper 时，额外运行：

```sh
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

本地暂未自动安装 pre-commit hook，手动运行检查；CI 检查以 [ci.yml](.github/workflows/ci.yml) 为准。已有 warning 不等于本轮新增问题，记录来源，避免在当前切片混入无关清理。

文档改动检查 Markdown 格式、相对链接、命令来源和 `git diff --check`；不为纯文档修改重跑录音、provider 或整个构建。macOS 可使用现有 Prettier 检查明确的文件（docs/ 被 Git 忽略，使用空 ignore 来源避免漏检）：

```sh
pnpm exec prettier --ignore-path /dev/null --check CONTRIBUTING.md docs/ENGINEERING.md docs/ARCHITECTURE.md
```

## 提交与 PR

- 使用 Conventional Commits，例如 `fix(dictation): prevent stale takes from pasting`。描述目的，一个提交围绕一件事。
- PR 使用 [现有模板](.github/PULL_REQUEST_TEMPLATE.md)，说明问题、实现、检查结果与未覆盖平台；行为或 UI 改动给出复现/验证步骤，必要时附截图或视频。
- 不将模拟 IPC、单测、构建成功写成真实录音或跨 App 验收。Windows CI 通过也不能代替焦点、快捷键、凭据存储等系统行为验证。
- 提交前自行阅读 diff，能解释 AI 辅助产生的代码和取舍；不理解的内容先讨论清楚。
- 不提交 API key、录音、完整听写正文、个人配置、证书或未脱敏的请求/响应日志。
- 用户可感知的改动按 [版本规范](docs/VERSIONING.md) 更新发版说明；内部文档修改标明无发版影响，不为此抬版本。

公共工程规则放在版本控制中的文档。维护者本地的 AGENTS/CLAUDE、产品草案及工具配置可能被 Git 忽略；贡献者不需要这些私有文件即可按本指南开发。
