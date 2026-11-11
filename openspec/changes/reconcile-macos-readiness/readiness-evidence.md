# macOS 收尾验收记录

日期：2026-11-11。生产代码基线：`d8ce2851ffb6918a5a91429ff34a06bd3e87b3cc`；本轮不修改产品代码、版本或依赖。

## 基线

- `pnpm check:engineering`：退出 0，229 个手写源码/测试/脚本文件通过。
- `pnpm test`：退出 0，47 passed、0 failed。
- `cargo test --manifest-path src-tauri/Cargo.toml`：退出 0，222 passed、1 ignored、0 failed；ignored 系统凭据测试未声称通过。
- [main CI 42982804213](https://github.com/auberginewly/Audie/actions/runs/42982804213)：d8ce285，双平台检查 success。CI 不代表 Windows 系统交互实测。
- [已发布 v0.2.1](https://github.com/auberginewly/Audie/releases/tag/v0.2.1) 已有 Apple Silicon DMG 与 Windows x64 setup.exe；本轮没有发版或覆盖已发布产物。

## 本地安装包

在既有隔离工作区执行，退出 0：

```sh
pnpm tauri build --target aarch64-apple-darwin --bundles dmg --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
```

- 本地 DMG：`src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/Audie_0.2.1_aarch64.dmg`。
- SHA-256：`fc3cd38077eac49f5ecf2a612243c646036a415253ad651a092a8e8d218b1e7d`。
- `hdiutil verify`：CRC VALID，退出 0。
- `hdiutil attach -readonly -nobrowse`：成功；用 `ditto` 复制包内应用到临时安装目录，再成功卸载镜像。
- 包内应用和安装副本均通过 `codesign --verify --deep --strict --verbose=2`；`file` 均为 Mach-O arm64。
- 安装副本 Info.plist：版本 0.2.1，标识符 `com.aubergine.audie`；`codesign -dv` 为 ad-hoc，未公证。本地验证包不是新的公开发行版。
- 临时安装副本已真实启动，主窗口可访问；启动前后 settings.toml 摘要相同，既有配置未改变。原 `/Applications/Audie.app` 为 0.1.1，保持原状；本次没有覆盖安装、回滚安装、真实数据迁移或声称完成正式升级验收。
- DMG bundler 成功后清理独立 `bundle/macos/Audie.app`；首次针对该路径的签名命令因路径不存在退出 1，后续改为验证只读挂载 DMG 内应用与真实安装副本，两者通过。

## 原生核心模式

先前已核对的普通 ASR 听写、录音中取消→撤销→实际粘贴见[上一轮证据](../harden-engineering-checks/native-evidence.md)。这些结果不作为真实 LLM 三模式通过的证据。

作者确认原 Audie 已配置 LLM。开发进程与本地发行包副本的 LLM 卡片均显示未配置；初次仅凭标签要求重新配置并不准确，随后已对照源码和无明文的凭据存在检查纠正判断：

- 当前 `settings.toml` 引用旧账户 `openai_compatible_api_key`；发行版 service 中该账户存在（Security presence 查询 status 0），新版卡片检查的 `openai_api_key` 不存在（status -25300）。开发版 service 中这两个账户都不存在。
- 新版 `pipeline/config.rs` 按已保存的 `llm_api_key_id` 读取，仍兼容旧账户；`models.ts` 的 OpenAI 卡片及连接测试使用卡片专属账户，不能用该卡片的“未配置”判定旧 LLM 配置丢失。
- 原 `/Applications` 发行版与临时发行副本标识符相同、使用同一应用配置目录；源码/旧包常量均使用发行版 service。没有复制、迁移、显示或记录任何明文密钥。
- 凭据存在和源码兼容不能证明实际读取获授权或网络调用成功。继续使用原配置做真实润色验收；卡片对旧账户的状态/测试兼容问题记作已发现缺口，本轮尚未改产品代码。

16:42:31，本地发行副本日志报告输入监控未授予，Fn 注册失败；主窗口可打开。已打开系统输入监控面板，请作者只完成测试副本的系统授权；无需重新填 key。不读取或迁移明文 key，不重置 TCC。

### 后续授权与启动核对

- 作者完成 macOS 身份验证后，从应用选择器选中已校验的临时 `installed/Audie.app` 加入输入监控；系统自动“退出并重新打开”实际启动了 `/Applications` 旧版。为避免双快捷键监听，已关闭该自动拉起的旧版，再明确启动临时副本。
- 19:38:19、19:42:46、19:48:11，原 ad-hoc 临时副本仍报告输入监控未授予；不能据系统列表开关为 on 推定这个进程已经获准。
- 保存原 ad-hoc 应用副本后，仅给本机测试副本改用项目现有 Apple Development 稳定身份；其 designated requirement 与既有开发二进制一致，签名完整性通过。原 DMG 与 `/Applications` 文件保持不变。该签名的测试副本通过 Launch Services 启动时，19:49:38 仍报告未授权。
- 随后从开发执行环境直接启动这份稳定开发签名的 **release 可执行文件**，19:51:09 日志确认 `trigger started (trigger)`、`registered trigger Fn`。它仍使用发行版凭据命名空间，并未改为 debug 服务或复制 key。
- 这证明本机开发启动方式下快捷键监听已注册，不能代替原 ad-hoc DMG 经 Launch Services 启动的系统授权验收。后者仍是已复现的待解决项，原因尚未最终确认；没有清空 TCC、删除原授权或覆盖原安装。
- 已打开受控 TextEdit 文档 `Audie-llm-acceptance-20261111.txt`（初始标题 28 字符），请作者配合实际口述。

### 真实 LLM 请求失败与原文兜底

- 19:52:13 → 19:52:24，Fn 启动/结束录音，音频 11178 ms、ASR final 29 字符；日志明确进入 `enhancing transcript with openai_compatible`。
- 19:52:26 返回 `OpenAI-compatible API key 无效，请检查设置`；对照 `llm/openai_compatible.rs::classify_http_status`，该提示仅映射 HTTP 401。调用失败后走原文兜底，19:52:27 `Processing -> Success (injected)`，19:52:33 回到 Idle。
- 受控 TextEdit 目标实际长度 28→57，原测试标题保留、测试用词“会议室”存在；与 29 字符 ASR 输出相符。**这是 LLM 失败后的原文粘贴，不是润色成功。** 未打印目标正文。
- 19:52:44 → 19:53:11 的第二次录音同样进入 LLM、收到相同 401 并报告注入后 Idle；没有该次受控目标的独立证据，不把它计为第二次目标粘贴验收。
- 作者反馈每次 Fn 都遇到需要密码的钥匙串提示。稳定开发签名的测试副本访问原发行版凭据，存在签名/访问授权边界；“允许本次”不会持久记住授权。并未读取 ACL、确认作者选择了哪种允许方式，不能宣称已经修复反复弹窗。
- 当前接口 host 为 `api.openai.com`、模型 `gpt-4o-mini`。密钥存在不等于该接口鉴权有效；已请作者确认实际服务商/域名，不索取密钥，暂不重复录音。
- 已在 Idle 后退出测试进程，重新打开未覆盖的 `/Applications/Audie.app` 原版。未复制/改写密钥、变更 endpoint 或清空 TCC。

| 项目                        | 本轮结果                                              |
| --------------------------- | ----------------------------------------------------- |
| 真正调用 LLM 润色并粘贴     | 真实调用返回 401，原文兜底粘贴已核对；成功润色未通过  |
| 选区改写并替换              | 待验收                                                |
| 独立快捷键写作              | 待验收；原 compose_hotkey 为空                        |
| 处理中取消/失败重试         | 已覆盖 LLM 401 的原文兜底；处理中取消和失败重试仍未测 |
| 安装副本重启后授权/配置保持 | 待完成系统授权和连接后验证                            |

## 文档和收尾

- STATUS 现状、VERSIONING、发布流程已对照源码/Git/真实发布状态更新；本机 ROADMAP 修复入口并保持 ignored，未加入公开仓库。
- `pnpm check:engineering` 及显式传入 VERSIONING、发布流程、ROADMAP、本 change 的四份 Markdown 均通过；229 个手写文件仍满足上限。
- STATUS 只格式化当前前缀并检查其中相对链接；附录按历史原样保留，经 `git show HEAD:docs/STATUS.md` 比较字节相同。历史附录的失效旧链接保留作历史，不作为当前实现入口。
- `openspec validate reconcile-macos-readiness --strict`、`git diff --check` 通过。
- 独立规格审阅确认范围/证据边界符合，指出测试目录被旧 Markdown 裸下划线误渲染的问题，改为代码目录 `__tests__`；修正后规格复核 PASS，独立质量审阅 PASS，无剩余发现。

提交时沿用既有 pre-commit 工程/TypeScript/lint/format/Rust fmt/clippy 门禁，不绕过。文档及包检查记录已以 `2d831b9` 提交、fast-forward 合入 main 并推送，既有 pre-commit 通过；该提交不包含后续本段的旧账户诊断补充。最终原生结果在本节继续补录；未覆盖项保持开放。toast 四态逐项真机走查沿用作者既有免除，不作为新增阻塞项。
