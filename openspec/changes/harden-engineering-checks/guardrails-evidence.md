# 工程检查与 hook 证据

2026-11-11；工作区 `codex/engineering-followups`，基线 `8bd115d`。本记录仅覆盖工程检查脚本及显式 hook 安装；CI 接入、其他源码改动及真实原生验收由主记录汇总。

## 实现范围

- `pnpm check:engineering`：用已有 Node 和 Prettier，检查 src、src-tauri、scripts、tests 与根目录手写 Rust/TS/TSX/MJS 文件的物理行数；400 行允许、401 行失败，空行/注释计入，末尾换行不另加行。仅排除实际依赖/构建目录及 src-tauri/gen；零发现失败，无例外名单。
- 文档默认清单固定为 CONTRIBUTING、ENGINEERING、ARCHITECTURE、两份 README。读取文件并加载仓库 Prettier 配置，独立于 Git ignore；用 Prettier 已有 Markdown parser 遍历链接/图片/引用定义及 HTML href/src，验证本地目标与 Markdown 标题锚点。外链不联网检查，历史 STATUS/OpenSpec 证据不默认格式化。可追加明确 Markdown 路径。
- `pnpm hooks:install`：仅显式安装，设置当前 clone 的 `core.hooksPath=.githooks`；重复运行安全，不写全局配置。已有其他 hooksPath（包括空配置）或默认 pre-commit（包括失效符号链接）时拒绝覆盖。
- `.githooks/pre-commit` 调用 Node runner：工程检查、typecheck、lint、format:check、Rust fmt、clippy --all-targets。首个失败停止并保留退出码，不能启动则失败；不改工作区/暂存区。`pnpm install` 不自动启用 hook。测试、build 与系统验收仍按改动类型运行。
- 没有新增依赖；package.json 仅增加两个 scripts；未修改测试发现器、业务源码或工作流。

## RED / GREEN

1. 先增加 14 个 CLI/fixture 用例；生产入口尚未存在时运行 `node --test tests/engineering-checks.test.mjs tests/hooks.test.mjs`，14 个失败。失败指向缺失 checker、installer、runner/hook；随后实现，14 个通过。
2. 新增空 hooksPath、失效默认 hook symlink 两个拒绝覆盖回归；实现前 `node --test --test-name-pattern 'explicitly empty|dangling' tests/hooks.test.mjs` 返回 1，两例均观察到错误的安装成功状态 `0 !== 1`。按实际配置 presence 与 lstat 修正后，两例及原有用例全部通过。
3. 最终 fixture 覆盖 400/401（业务、测试、脚本）、生成/依赖输出排除、被忽略文档的格式/损坏链接与锚点、Unicode/重复标题/引用链接、额外文档参数、零发现、安装幂等、用户配置保护、子进程失败退出码及 shell hook 传播。所有 Git 配置写入均发生于临时 fixture 仓库。

## 本轮检查

| 命令                                                                                                                                                                            | 实际结果                                                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `node --test tests/engineering-checks.test.mjs tests/hooks.test.mjs tests/release-version.test.mjs`                                                                             | 24 passed，0 failed；18 个新增工程/hook 用例及 6 个原发布版本用例                                                             |
| `pnpm test`                                                                                                                                                                     | 发现 10 个测试文件，45 passed，0 failed；包含同时进行的诊断隐私测试                                                           |
| `pnpm check:engineering`                                                                                                                                                        | 通过；229 个手写源码/测试/脚本文件与全部默认指南                                                                              |
| `pnpm exec eslint scripts/check-engineering.mjs scripts/install-hooks.mjs scripts/pre-commit.mjs tests/engineering-checks.test.mjs tests/hooks.test.mjs`                        | 退出 0                                                                                                                        |
| `pnpm exec prettier --check scripts/check-engineering.mjs scripts/install-hooks.mjs scripts/pre-commit.mjs tests/engineering-checks.test.mjs tests/hooks.test.mjs package.json` | 退出 0                                                                                                                        |
| `pnpm format:check`                                                                                                                                                             | 本轮并行工作期间退出 1，仅报告其他负责人的 `src/lib/__tests__/diagnostic-privacy.test.mjs` 待格式化，已通知协调者；集成时重跑 |
| `git diff --check`                                                                                                                                                              | 退出 0                                                                                                                        |

本子任务没有在真实项目安装 hook，也没有提交或修改 index。真实仓库 `git config --local --get core.hooksPath` 返回 1（未设置）；主任务按已批准流程合并后显式安装并验证。Windows shell hook 与 symlink 直接执行用例仅在非 Windows 执行；Node runner 包含 Windows pnpm.cmd 接线，Windows 系统行为以 CI/实测为准。
