# Why

前端同一功能分散在 components、hooks、types、store；历史页面与多语言文件混有多个职责，作者需要跨目录追踪。执行作者已批准的总体 [refactor-module-organization 方案](../refactor-module-organization/design.md) 中全部 S7 前端组织范围。

# What Changes

- 将现有实现移至 dictation、history、home、providers、text、hotkeys、audio、permissions、settings、onboarding 十个功能；公共 UI、shell 和平台小工具保留。
- SettingsSchema 保持完整设置合同；provider / 设备 / 历史 / 听写与预览事件合同归各功能。三个提示词字段仍归设置。
- App 保留组合与导航，向导协调移至 onboarding；Capsule 拆波形、终态展示；HistoryScreen 拆条目展示；文案按语言拆分。
- 现有 Node 测试就近迁移，增加跨平台发现入口，扩展 mjs lint / format 范围，更新定向脚本。

# Scope / Approval

本片范围已获作者“完成全部剩余代码组织”的明确授权；这是已批准组织方案的实施记录，不再增加产品能力或架构决策。恢复多模板、循环键、新依赖、新框架、UI / CSS 改版均不在范围。Rust 与 STATUS 由协调者负责，本片不触碰。

# Capabilities

无产品能力变更；skip_specs: true。不向产品 specs 虚构新行为。

# Validation

比较迁移前后 26 个测试身份与三种语言全部有效 key/value；运行默认及定向测试、typecheck、lint、format:check、build。记录实际浏览器冒烟与原生验证边界。
