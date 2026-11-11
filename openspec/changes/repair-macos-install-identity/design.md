# Design

## 诊断

21:49:02 系统日志报告临时 app 的 ListenEvent/Microphone `SecStaticCodeCheckValidity` 失败（-67050）。日志中的静态代码摘要与旧 `/Applications` 的 ad-hoc 0.1.1 一致，与临时稳定签名 app 不一致。推断同 bundle id 的副本和旧安装触发了身份缓存混用，需要固定安装路径验证，而不是改动权限判断代码或重置 TCC。

## 实施

采用固定路径安装、稳定本机签名的方案；继续改临时副本签名不会解决 canonical 路径中的旧代码。使用 Tauri 官方 APPLE_SIGNING_IDENTITY 入口，证书值只从本机私有配置传给构建进程，不写仓库。新包已完成编译与 codesign 严格验证，DR 与当前可用副本一致。

先复制并验证新 app 的安装暂存目录；等待现有处理进入 Idle 后退出 Audie。将原 app 完整移入本机私有备份目录，再将暂存包移到 `/Applications/Audie.app`；出错时恢复原 app。只刷新这个 app 的 Launch Services 注册并从固定路径启动。安装前后核对非敏感配置摘要；不复制、读取、迁移明文密钥，不清空系统授权。

本地开发证书仅用于当前 Mac 验证，不将它用于公开发行。现有 ad-hoc CI 分发仍有签名身份不稳定的限制；正式 Developer ID 与公证是独立发行工作，不能将此次本机修复宣称为公开安装包已公证。
