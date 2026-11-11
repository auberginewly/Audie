# 集成验收记录

2026-11-11；基线 8bd115d，隔离分支 codex/engineering-followups，主线按分项提交 fast-forward 合并。没有新增依赖、修改 lockfile 或增加架构层。

## 本地实际检查

| 检查                                        | 结果                                                                         |
| ------------------------------------------- | ---------------------------------------------------------------------------- |
| Rust 全量测试                               | 222 passed / 0 failed / 1 ignored（会修改用户钥匙串的原测试继续忽略）        |
| Rust fmt / clippy --all-targets -D warnings | 通过                                                                         |
| pnpm test                                   | 47 passed / 0 failed；含 18 个新工程检查/hook 用例及 3 个前端诊断用例        |
| typecheck / lint / format:check / build     | 通过；lint 0 error / 11 条已有模式 warning                                   |
| pnpm check:engineering                      | 229 个手写源文件/测试/脚本均 ≤400 行；全部五份公共指南通过格式和相对链接检查 |
| pnpm tauri build --debug --no-bundle        | 通过；在本机启动 debug 原生应用，独立记录系统验收                            |
| OpenSpec strict / git diff --check          | 通过                                                                         |

仍有依赖 block 0.1.6 的 future-incompatibility 提示，本片不为此升级依赖。隐私/规模检查的 RED、fixture 负对照及 GREEN 见分项记录；pipeline 的 76 个函数/测试正文与基线一致。

## 审查

分项独立只读方案/质量审查覆盖构建/runner、工程脚本/hook、pipeline、provider 与前端诊断，均无剩余可操作问题。按审查意见明确“处理中取消的转写尚未返回时，立即撤销可能重新请求 ASR”，只改注释。

## 提交与本地 hook

- d6eea84：修复 Windows libtests 缺 manifest 及退役 macOS runner；已推送且远端 Windows/macOS 检查通过。
- 839c352：记录已批准的范围与实际隐私规范。
- b86c2b8：收口诊断隐私，保留错误分类和合法转写。
- 350275f：规模/指南检查、显式 hook 和 CI 接线。
- abb12dd：pipeline 显式依赖和中文 why 注释。

已在作者主 clone 显式运行 pnpm hooks:install，实际 core.hooksPath=.githooks；随后从主工作区直接运行 .githooks/pre-commit，工程检查、前端静态检查、Rust fmt/clippy 全部 exit 0。安装不作为 pnpm install 的隐式副作用；其他 clone 仍需显式启用。

原生范围与剩余真实场景见 [native-evidence.md](native-evidence.md)；远端构建故障证据见 [ci-evidence.md](ci-evidence.md)。Windows 系统操作仍需真实 Windows 设备，CI 不替代系统验收。

整合后的生产代码已推送 main=abb12dd，[对应 CI 42979405201](https://github.com/auberginewly/Audie/actions/runs/42979405201) 已完成：Windows 与 macOS quality 均为 success，包含本轮完整生产代码和工程检查接线。收尾文档提交后将再次确认远端状态。

收尾审查再次独立执行主 clone 工程检查及 21 个隐私/工程 fixture，全部通过，无剩余源代码或安全问题。真人口述反馈待补，原生完整矩阵继续开放。
