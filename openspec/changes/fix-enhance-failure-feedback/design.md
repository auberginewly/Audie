# Design

采用前面已讨论、作者要求实施的“现有事件携带实际原因”方案。仅改静态文案不能帮助区分鉴权、网络和模型错误；新增错误事件或状态没有必要。

- managers/enhance.rs：兜底结果保留 AppError 的用户可见原因，仍使用原文。
- pipeline/context.rs、processing.rs：现有 Failed 结果带原因，注入完成后通过原 enhance-progress.message 发布。
- features/dictation/Capsule.tsx：标题固定为已插入原文，副标题显示失败原因；旧事件没有原因时用通用失败提示。
- i18n/locales：通用失败提示不再断言缺少模型配置。
- 就近 Rust 测试及 dictation/**tests**：先证明 401/网络错误不能变成“未配置”，再验证原文注入、历史分类和取消隔离仍成立。

不新增依赖、架构层、事件字段或凭据读取；不返回 API 响应正文。代码单元检查后构建，并在运行应用中验证受控失败反馈。
