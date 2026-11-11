# 诊断隐私验证证据

日期：2026-11-11。实现沿用现有 provider、AppError 与前端订阅，无新依赖或运行时抽象。

## 范围与边界

- OpenAI / GLM / StepFun / OpenAI-compatible / provider 测试的 HTTP 分类保留原有 Provider/Network 分类、固定可操作说明及必要状态码，不再回显响应片段。
- 读取、JSON/SSE、WebSocket、鉴权头与 endpoint 失败不再格式化底层错误；Doubao 不回显 endpoint/resource_id，保留数字服务错误码。
- Aliyun / StepFun / Doubao 服务端错误不把 message/body 放入错误对象或返回的 AppError；Doubao codec 的 Display/Debug 也不含收到的错误正文。
- 前端控制台只接收固定诊断类别，移除 raw payload、Zod error 和未知异常；保留 catch、订阅、清理、状态镜像、用户错误反馈与合法转写内容。
- 既有测试均保留；仅原来明确要求回显 provider 正文的断言改为固定说明/状态码。

## RED → GREEN

修改实现前运行 `cargo test --manifest-path src-tauri/Cargo.toml privacy_ -- --nocapture`：14 failed，0 passed。失败分别来自五类 HTTP body、五类 reqwest 凭据/query URL、GLM 无法解析正文、StepFun SSE 错误、Aliyun 服务端错误/未知事件与 Doubao WebSocket 上下文。随后同一组用例 14 passed。

前端新增测试执行源码中的 console 表达式，将未知异常、Zod error 和 payload 放入私有 marker。修改前在 App 的原始 err 日志处失败；移除原始参数后通过。进一步运行实际 `useRecordingFlow` 回调：三类非法 event 均被拒绝且日志无 marker；合法 event 仍写入三个原 store setter；订阅完成前卸载仍逐个清理；订阅拒绝仍记录固定诊断且不写状态。

补充 localhost HTTP fixture 验证真实 LLM/provider model-list 失败响应及 JSON decode；成功转写/润色仍原样返回 marker，证明隐私收口仅影响诊断。

## 本轮命令结果

| 检查                                                                                   | 实际结果                                                        |
| -------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `cargo test --manifest-path src-tauri/Cargo.toml`                                      | 222 passed / 1 ignored / 0 failed；含原恢复回归                 |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`       | exit 0                                                          |
| `cargo fmt --manifest-path src-tauri/Cargo.toml`                                       | exit 0                                                          |
| `pnpm test`                                                                            | 47 passed / 0 failed（包含其他工程检查切片测试）                |
| `node --experimental-strip-types --test src/lib/__tests__/diagnostic-privacy.test.mjs` | 3 passed                                                        |
| `pnpm typecheck` / `pnpm build`                                                        | exit 0                                                          |
| `pnpm lint`                                                                            | 最终结果见协调者整体检查；此前生产代码检查 0 error / 11 warning |
| `pnpm format:check` / `pnpm check:engineering`                                         | exit 0；229 个源码文件符合规模限制                              |
| `git diff --check`                                                                     | exit 0                                                          |

Rust 日志位于本机 `/tmp/audie-privacy-cargo-test.log` 与 `/tmp/audie-privacy-clippy.log`，前端测试位于 `/tmp/audie-privacy-frontend-test.log`。日志不作为跨机器可访问附件；上表记录本轮实际结果。

本项未访问真实密钥、音频或历史，未调用真实云 provider。ignored Keychain 测试保持未运行；真实原生、跨 App 注入与远端 CI 由整体切片另行记录，不能由这些单测代替。
