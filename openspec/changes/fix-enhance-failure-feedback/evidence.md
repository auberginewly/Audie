# Evidence

## 基线与失败复现

基于 e7f69d9。原生 DeepSeek 401 与截图表明：pipeline 丢弃错误原因，Capsule 对全部润色失败固定显示“未配置润色模型”。

- Rust 先改兜底断言：鉴权与网络原因各一条，旧实现均失败，实际仍为固定“润色失败但已注入原文”。
- 前端先渲染真实 Capsule/CapsuleToast：鉴权、额度、网络和空原因四条断言均在旧实现失败。初次测试缺少音量 hook mock，修好测试边界后重新确认四条是显示内容断言失败，而非模块加载错误。

## 检查结果

- Rust 全库：223 passed / 1 ignored，包含取消/恢复及错误分类隐私回归。
- 前端全量：51 passed；修复后再次单跑真实胶囊渲染 4/4 通过。
- typecheck、工程检查（230 个手写源码）、clippy all-targets -D warnings、严格 OpenSpec 校验通过。定向 lint 只有 Capsule 原有 setState-in-effect warning；本轮没有新增 warning。
- 额外把两份历史长文档整体纳入格式/链接扫描时，发现原有格式差异及 STATUS 附录中的旧失效链接；已核对新增段落格式，附录保持字节不变。这不属于默认公共指南检查的通过范围。
- 独立代码/方案审查 PASS，无剩余发现。失败历史仍不存 enhanced_text，generation 检查和终态时序保留。
- 使用现有 Apple Development 身份构建完整 release .app，codesign 严格验证通过，标识符和 DR 与原可用稳定副本一致。固定安装后的原生结果见下节。

## 原生安装包验证

在 `/Applications/Audie.app` 正常系统启动、重新绑定本机授权后，用本机生成的短测试语音进行一次真实录音；没有将浏览器模拟当原生通过。

- 22:10:21 Recording，22:10:30 Processing，9301 ms 音频得到 17 字符 final。仅临时修改官方 DeepSeek host 下的路径来触发真实 HTTP 404，未变更密钥或模型。
- 22:10:31 日志为 provider 拒绝请求（404）并原文兜底注入，22:10:37 Idle。
- 真实原生 overlay 的 AX 文本为“已插入原文”和“OpenAI-compatible 拒绝请求（404），请检查模型与连接设置”，未出现硬编码“未配置润色模型”。作者也确认原文已粘贴、显示具体错误。
- 受控历史记录 99 的 enhanced_text 为空；TextEdit 内容等于测试标题加 17 字符转写原文，证明当前安装包的实际粘贴链路成功。没有打印正文或读取无关历史。
- 故障脚本在 finally 恢复原 endpoint，最终 TOML 核对一致。初次等待真人录音超时也已通过 finally 恢复，未将超时计为通过。
