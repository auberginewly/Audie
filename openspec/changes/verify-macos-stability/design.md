# 验收方式

验收记录在本 change 和 docs/STATUS.md；复现后最小修复只涉及 platform/macos/clipboard.rs 与独立 clipboard/tests.rs。测试脚本放临时目录，通过现有系统快捷键及原生 UI 操作应用；通过目标字段与本次成功历史逐字比较，区分润色结果和原文兜底，不以 Success 日志单独判定粘贴成功。

每次仅对明确创建的本地测试文档/页面操作；不读取原聊天、用户文档或无关历史，不发送消息。使用合成短语音经扬声器、真实麦克风和正常 provider 路径测试，明确区别于真人口述。

ASR 临时故障修改既有官方 host 的路径；LLM 连接故障使用本机绑定但未监听的端口，先清空临时请求的 key 引用，避免把真实凭据发给回环端口。两者均 try/finally 恢复、核对设置字节一致；不改密钥、不关闭整机网络，不重置权限或签名。睡眠唤醒需要作者实际操作。遇到源码 bug，先写复现和最小修复方案，再在既定结构内处理。

## 空选区与复制整行

macOS 仅查前台控件的 AXSelectedText，不读文档全文。Some(空串) 是确定没有选区，必须与未知 None 区分；已知文本直接使用，不触碰剪贴板。AX 不支持的普通应用仍保留原 Cmd+C 回退。

Electron 的 AX 树按需创建：首次查询失败时通过其公开 AXManualAccessibility 开关启用，立即再查一次，不增加 sleep。若仍未准备好，保守当作空选区，避免复制整行误改写；这一瞬间的真实选区可能按普通听写处理，之后 AX 可用便恢复自动改写。每次 AX 请求限定 50ms，避免不响应的外部应用长时间堵住热键回调。CF Copy/Create 返回值按 RAII 释放，类型校验后才向 AX 对象发请求。

参考：Handy 的 clipboard 仍是系统层边界；其版本没有 Audie 的选区改写能力，不能照搬其缺少的选区判定。Voxt 仅核对 AX 读取思路，未抄 Swift。依据 [VS Code 快捷键](https://code.visualstudio.com/docs/reference/default-keybindings)、[Electron 辅助功能](https://github.com/electron/electron/blob/main/docs/tutorial/accessibility.md)、[Apple AXSelectedText](https://developer.apple.com/documentation/applicationservices/kaxselectedtextattribute)。
