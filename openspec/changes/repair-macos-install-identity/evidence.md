# Evidence

## 身份诊断

TCC 系统日志的静态摘要对应旧 `/Applications/Audie.app` 的 ad-hoc 签名，实际临时副本是不同摘要的 Apple Development 签名。ListenEvent 与 Accessibility 均报告 code requirement 不匹配。仅给临时副本开关授权不能证明 app 正常启动可用。

## 固定安装

- 已构建 0.2.1 本机开发签名包，非公开发行。应用包含 microphone usage 描述，标识符 `com.aubergine.audie`，DR 与本机可用副本相同。
- 先复制完整安装暂存包并通过严格签名验证。第一轮安装检查发现两份旧测试进程，自动中止且未改原安装；核对它们均为本任务临时副本后退出，再继续安装。
- 原 0.1.1 完整备份于 `~/Library/Application Support/Audie-install-backups/20261111-215413/Audie.app`，未删除。新 app 安装到 `/Applications/Audie.app`；安装前后 settings.toml SHA-256 相同。没有读取、迁移或输出明文凭据。
- 只刷新 canonical app 的 Launch Services 注册，启动进程确为 `/Applications/Audie.app/Contents/MacOS/audie`。21:54:15 仍缺输入监控，系统日志继续报告旧 code requirement 不匹配，因此没有将安装成功计为权限修复完成。
- 只切换大写 Audie 项 off/on 并重启，21:55:48 仍失败，说明仅开关并未修复旧授权身份。随后在系统输入监控中移除大写 Audie 旧项并点击添加，系统随后直接出现应用选择窗口，选择 canonical app；小写 audie 及其他应用项未改动。未清空全机 TCC。

## 授权与真实链路结果

- 移除旧输入监控项后重新选择 `/Applications/Audie.app`，系统正常启动日志 22:00:53 确认 Fn 注册。仅开关时失败、移除重新添加后通过，明确保留了前面的失败记录。
- 在辅助功能列表同样只重新添加大写 Audie；随后再次从 Launch Services 正常启动，22:02:49 Fn 再次注册。进程路径为 canonical app，没有使用终端直接 exec 继承父进程授权。
- 重启后的 DeepSeek 配置界面实显“连接测试通过 · 237ms”，使用保存的凭据；没有要求作者重新填写 key。系统选择器已可直接操作，不将先前询问身份验证视为实际发生或完成密码验证的证据。
- 22:10 的原生错误反馈案例证明麦克风录音、ASR、LLM 失败兜底及实际 TextEdit 粘贴；目标与受控 raw 逐字一致，作者确认显示具体错误。见 [原生反馈证据](../fix-enhance-failure-feedback/evidence.md)。
- 最终严格验证备份旧 0.1.1、新安装 0.2.1 签名完整；正常 endpoint 已恢复，临时写作键仍为空，主键仍 Fn。没有更改发布版本号、GitHub Release 或 CI 的 ad-hoc 策略。

这次修复的是维护者本机固定路径、同一稳定开发身份的安装和授权。未经签名的公开包、其他用户机器、未来证书更换或正式公证不在已验证范围。
