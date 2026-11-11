## ADDED Requirements

### Requirement: Provider failures use safe diagnostics

Provider 失败 SHALL 保持现有错误分类与安全的状态码/阶段信息，但 SHALL NOT 将响应正文、录音/听写/提示词/选区正文、API key、Authorization 或带凭据/query 的 URL 拼入返回错误和诊断日志。

#### Scenario: Provider returns content containing private markers

- **WHEN** provider 返回携带私有 marker 的错误或不可解析响应
- **THEN** 返回错误包含稳定分类和固定可操作说明，且不包含 marker、原始 body 或凭据 URL

### Requirement: Frontend validation diagnostics omit raw inputs

前端外部数据校验失败 SHALL 记录固定类别或安全计数，SHALL NOT 输出完整 payload、可回显输入的校验错误对象或原始未知异常。

#### Scenario: Malformed event contains private user text

- **WHEN** 接收到含私有 marker 的非法 Rust event
- **THEN** 数据被拒绝且诊断输出不包含该 marker，同时订阅清理/状态镜像行为保持
