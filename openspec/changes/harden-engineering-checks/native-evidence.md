# macOS 原生验收记录

2026-11-11；macOS 26.4，Apple Silicon。在本机执行 pnpm tauri build --debug --no-bundle，通过后经主仓已有 dev-sign-run.sh 使用稳定开发签名启动新构建 target/debug/audie，codesign --verify --strict 通过。没有可调用的 Reticle 工具，因此本记录是实际原生 Accessibility/系统日志观察，不标为 Reticle 验证，也不将浏览器 mock 当原生证据。

## 已观察到的真实行为

- 主窗口实际渲染 Home、设置入口和首次配置向导，AX 可访问按钮；关闭向导成功。Fn 快捷键实际注册，应用选择 MacBook Pro 内建麦克风并成功打开 48000 Hz 单声道采集。
- 经系统 CGEvent 发送测试 Fn，真实状态 Idle → Recording；点击胶囊取消，Recording → Cancel → 六秒后 Idle，没有注入。
- 录音中取消后，在六秒内点击“撤销操作”，实际 Cancel → Processing，重新建立 ASR WebSocket；空识别进入 Error → Idle。此案例证明恢复可执行，不证明有效文字恢复/粘贴。
- 固定系统测试语音尝试有多轮返回“没有识别到内容”，AX 观察该中性提示，Error → Idle 正常；已询问作者配合真人口述，不用假转写替代。
- 一次真实流式结果 final=true、chars=5，进入 Processing → Success → Idle；该次受控 TextEdit 目标内容未增加，因此仅记录 Rust 报告注入成功，不能宣称目标字段粘贴已验收。

## 用户配合验收

作者已确认在电脑前，可以配合录一句。已提供临时文件 /tmp/Audie-acceptance-20261111.txt，并请作者在末尾用 Fn 口述固定测试句、Fn 结束；结果待回报和目标内容核对。配合期间不再自动抢焦点或发送热键。

## 仍需明确证据的边界

有效口述确实粘贴到目标字段、LLM 润色、三模式、有效内容的取消/撤销/失败重试/原文插入、✓/✕真实鼠标点击不抢焦点、多屏/Space、设置与设备预览交接、重启迁移/钥匙串和 Windows 实机尚未全部验收。原 OpenSpec 系统任务保持开放；不以代码事件 Success 或单测通过替代目标窗口结果。

未读取/重写密钥、真实提示词和历史内容，未重置 TCC；真实 ASR 由现有应用按需使用原配置和钥匙串。测试目标只创建临时验收文件，未主动编辑其他文档；未将不确定的注入落点判为通过。应用仍保留运行供作者验收。

本轮签名后测试二进制 SHA-256：b3e546931257986d5633a11e963c23495402f94a91946238493b477658dd2abf。
