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

### 修正服务商配置后复测

- 作者随后确认原密钥属于 **DeepSeek**；当时本机配置却是 `https://api.openai.com/v1` + `gpt-4o-mini`，二者不匹配。先前 OpenAI 接口的 401 不能据此推定 DeepSeek 密钥本身失效。
- 对照 [DeepSeek 官方首个 API 调用](https://api-docs.deepseek.com/) 和 [模型列表文档](https://api-docs.deepseek.com/api/list-models/)，只将本机 `settings.toml` 的 `openai_compatible_base_url` 改为 `https://api.deepseek.com`、`openai_compatible_model` 改为 `deepseek-flash`。
- 修改前私下备份完整本机配置；TOML 解析对比确认仅上述两项不同，原 `llm_api_key_id = openai_compatible_api_key` 与其他设置不变。没有读写、迁移或显示明文 key；本机配置不入 Git。
- 原应用历史页对刚才受控测试记录 77 发起“重试润色”，但未取得可确认成功的结果，受控行 `enhanced_text` 仍为空；不作为 DeepSeek 连接成功或最新代码验收证据。未操作其他历史记录。
- 随后退出原版，再直接启动同一份、签名不再变化的最新测试 release 可执行文件。20:12:05 日志确认 Fn 注册成功。
- 新受控 TextEdit 文档为 `Audie-deepseek-acceptance-20261111.txt`。已说明系统钥匙串的“允许”只针对本次、“始终允许”才记住当前身份；不同凭据条目可能分别询问。作者配合实际复测后的结果继续补录，不能以改好配置替代成功调用。
- 20:13:58 进入 DeepSeek 请求，20:13:59 仍返回映射 HTTP 401 的鉴权错误并回退原文。当时仍引用旧账户；作者最初反馈文字已粘贴，随后报告“未配置润色模型”，不能将前一个反馈计为润色成功。
- 后续 presence-only 查询确认发行版及开发版各自的 `deepseek_api_key` 条目均存在；没有读取密钥内容。仅将本机 `llm_api_key_id` 改为 `deepseek_api_key`，TOML 比较确认该次只有这个字段变化。
- 对受控历史记录的重试未取得成功证据，且期间出现多次 enhance 调用。已停止重试并在 20:22 清理测试进程、直接重启同一稳定签名的 release；不将这些调用记为成功或失败恢复验收。
- 20:23:15 开始录音，20:23:26 处理 10261 ms 音频、27 字符 ASR final；使用 DeepSeek endpoint/model/key 引用的调用仍返回同一 HTTP 401，随后回退原文、20:23:32 Idle。没有独立核对该次目标应用正文，不只凭日志认定受控目标粘贴通过。
- 作者截图显示“润色失败但已注入原文 / 未配置润色模型”。源码核对发现 `Capsule.tsx` 对所有失败都固定使用 `overlay.polishMissing`，而 processing 丢弃具体失败原因；因此截图不能说明实际缺少模型配置。该产品缺口尚未修复，不属于本 change 原先的文档与验收范围。
- 作者进一步确认密钥来自 DeepSeek 官方开放平台，已打开测试应用的 DeepSeek 配置窗口，请作者在应用内核对/更新有效密钥并测试连接；暂停重复口述。密钥是否有效、反复钥匙串询问是否解决，均未获得成功证据。

### 有效密钥保存后的成功验证

- 作者在 DeepSeek 配置窗口重新填写密钥，反馈“测试通过，已保存”。只核对了已保存的非敏感字段：官方 endpoint、`deepseek-flash`、`deepseek_api_key`；未读取密钥。
- **润色通过**：20:35:04 开始录音，20:35:14 处理 9834 ms 音频，ASR final 38 字符；20:35:16 注入后回 Idle，无润色失败日志。受控历史记录 81 的 mode 为 polish，`enhanced_text` 非空且为 26 字符，与 38 字符原文不同。
- 受控 TextEdit `Audie-polish-verified-iw5vg_sw.txt` 实际从 26→52 字符，原标题完整保留，新增部分与该记录的模型输出逐字一致；作者反馈自动粘贴且无错误。由模型结果与目标比对确认成功，不只凭 Success 状态判断。
- **选区改写通过**：受控 `Audie-rewrite-verified-er27coym.txt` 仅包含 22 字符测试原文，已全选。作者口述要求改为正式会议通知；20:36:26 开始，20:36:32 转写 5952 ms 音频、24 字符指令，20:36:33 注入后 Idle，无润色失败日志。
- 受控历史记录 82 的 mode 为 rewrite，历史 raw 包含原选区，模型输出 30 字符；目标从 22→30 字符且全文与模型输出逐字一致。作者确认选区已被会议通知替换。没有打印目标原文、模型正文或无关历史。
- **独立写作通过**：作者在原生录键窗口设置临时 `Ctrl+Alt+Cmd+K`，核对本机配置已保存，Fn 主键不变。20:39:48 开始写作录音，20:39:57 处理 9226 ms 音频、37 字符口述要求，20:39:58 注入，20:39:59 Idle。
- 受控历史记录 84 的 mode 为 compose，模型返回 64 字符且与口述不同；`Audie-compose-verified-0gf9c49z.txt` 从 27→91 字符，标题保留，新增部分与模型输出逐字一致，作者确认已生成会议通知并自动粘贴。
- 同一测试准备期间 20:39:37 曾发生 Recording→Processing→Error，20:39:43 自动回 Idle；没有把这次错误行算成写作成功，核对仅使用随后 9226 ms 的 success 记录。
- 本次只证明当前稳定开发签名直接运行的 release 在本机的成功路径；不扩展为原 ad-hoc 安装包授权、重启免密码或其他尚未测矩阵通过。

### 处理中取消、撤销与配置恢复

- 使用独立写作键及新的受控 TextEdit `Audie-processing-cancel-pw3zdyxd.txt`；作者口述结束，观察真实日志进入 enhance 后，在现有胶囊点击取消一次。没有注入测试 hook 或修改产品代码。
- 日志：20:41:40 Recording，20:41:46 Processing，20:41:47 Cancel；保持约 4 秒后目标仍与原始标题全文一致，证明取消期间没有旧结果粘贴。
- 随后点击“撤销操作”一次：20:41:51 Cancel→Processing→Success，20:41:52 Idle。成功历史记录 86 仍为 compose，口述 34 字符、模型输出 48 字符，目标全文等于原标题加模型输出。作者确认先取消、随后恢复并粘贴通知。
- 该次恢复证明写作模式保留、取消后的旧结果未注入；没有证明旧 6 秒定时器在新 Processing 期间触发，因为恢复已在旧定时器之前完成。ASR 是否重发不只凭重复的 transcription 日志推断。
- 验收后将本机 `compose_hotkey` 恢复原空值；TOML 解析比较确认只变这个字段，Fn 和用户刚保存的 DeepSeek 设置保留。退出旧测试进程，再直接启动同一稳定签名的 release 副本；20:42:19 Fn 注册成功，没有重新签名或覆盖安装。
- 重启后从 DeepSeek 配置窗口点击现有“测试”，原生界面实际显示“连接测试通过 · 192ms”，该次没有要求作者重新填写密钥或协助授权。证明本次重启保存的凭据可用，不推定以后所有启动身份都不会触发钥匙串，也不代替原 ad-hoc DMG 的授权问题验证。

### 真实转写错误后的重试

- 新受控 TextEdit `Audie-error-retry-eyyhxt1e.txt` 全选 24 字符测试原文；Fn 应进入 rewrite。临时只改本机 `doubao_endpoint` 的路径为不存在的测试路径，host 仍为原官方服务；未改密钥、系统网络或其他用户配置。故障脚本无论成功、失败或超时均在 finally 恢复原 endpoint。
- 20:44:19 Recording，20:44:27 Processing→Error；实际 provider 错误为 `Network("doubao handshake failed (HTTP 404)")`。错误期间读取受控目标，全文仍是原选区，未粘贴。
- 自动恢复原 endpoint 后，仅点击现有胶囊“重试”一次；20:44:28 Error→Processing，20:44:31 用保留的 7936 ms 音频取得 27 字符转写，20:44:32 Success→Idle。没有再次录音。
- 成功记录 87 的 mode 为 rewrite，raw 仍包含原测试选区，模型输出 27 字符；目标全文与模型输出逐字一致，证明此次错误重试保留音频及改写模式/选区。未读取或输出无关历史内容。
- 作者要求重复一次：20:45:38 Recording，20:45:44 HTTP 404→Error，20:45:45 恢复配置并重试，20:45:47 Success、20:45:48 Idle。记录 89 为 rewrite，目标与模型输出一致；但作者反馈“感觉好像没替换”。重新核对发现模型输出与 24 字符原选区逐字相同，因此这轮只记录错误恢复路径，不将其当成可见改写效果通过。没有用后台 Success 否定作者观察。
- 为消除“结果未变化”歧义，第三次使用明确修改时间和地点的口述要求，受控文档 `Audie-error-retry-visible-gwmd6420.txt`。20:47:17 Recording，20:47:22 HTTP 404→Error；恢复原路径并点击重试后同秒进入 Processing，20:47:27 用保留的 5376 ms 音频转写 25 字符指令，20:47:28 注入后 Idle。
- 记录 90 仍为 rewrite，历史 raw 含原选区；模型输出 25 字符，与原文不同且明确包含“后天”、上午十点及二号会议室，目标全文与模型输出一致。作者确认新时间、地点已出现。错误期间目标原文保持不变，无需重新录音；至此同时具备错误恢复与可见改写结果证据。
- 最终再次解析配置，确认豆包 endpoint 已恢复、主键仍 Fn、临时写作键仍为空。这个案例验证受控 HTTP 404 后恢复，不扩展为所有网络错误、注入失败“插入原文”、跨 App 焦点、多屏/Space 或 Windows 验收完成。

| 项目                        | 本轮结果                                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------- |
| 真正调用 LLM 润色并粘贴     | 有效密钥保存后通过，模型输出与 TextEdit 实际粘贴逐字一致                                          |
| 选区改写并替换              | 通过，原选区被模型结果替换，目标全文一致                                                          |
| 独立快捷键写作              | 通过，临时独立快捷键触发 compose，生成内容与目标实际粘贴一致                                      |
| 处理中取消/失败重试         | 写作处理中取消→撤销、转写 HTTP 404→恢复配置→重试改写均通过，目标核对一致                          |
| 安装副本重启后授权/配置保持 | 稳定开发签名直接启动的 release 重启后 Fn/已存凭据测试通过；原 ad-hoc Launch Services 授权仍未通过 |

## 文档和收尾

- STATUS 现状、VERSIONING、发布流程已对照源码/Git/真实发布状态更新；本机 ROADMAP 修复入口并保持 ignored，未加入公开仓库。
- `pnpm check:engineering` 及显式传入 VERSIONING、发布流程、ROADMAP、本 change 的四份 Markdown 均通过；229 个手写文件仍满足上限。
- STATUS 只格式化当前前缀并检查其中相对链接；附录按历史原样保留，经 `git show HEAD:docs/STATUS.md` 比较字节相同。历史附录的失效旧链接保留作历史，不作为当前实现入口。
- `openspec validate reconcile-macos-readiness --strict`、`git diff --check` 通过。
- 独立规格审阅确认范围/证据边界符合，指出测试目录被旧 Markdown 裸下划线误渲染的问题，改为代码目录 `__tests__`；修正后规格复核 PASS，独立质量审阅 PASS，无剩余发现。

提交时沿用既有 pre-commit 工程/TypeScript/lint/format/Rust fmt/clippy 门禁，不绕过。文档及包检查记录已以 `2d831b9` 提交、fast-forward 合入 main 并推送，既有 pre-commit 通过；该提交不包含后续本段的旧账户诊断补充。后续三模式、处理中取消/撤销与转写失败/重试结果现已补入本节；原 ad-hoc 授权和未覆盖项保持开放。toast 四态逐项真机走查沿用作者既有免除，不作为新增阻塞项。
