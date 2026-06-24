# macOS 代码签名与发布 · 你看的

> 这份给你自己看的，从**原理**讲到**发版**。你说过不太懂 macOS 签名——读完这份就够用了。
> 实操配置散在 `.cargo/config.toml`、`scripts/dev-sign-run.sh`、`src-tauri/tauri.conf.json`，这份只讲「为什么」和「发版该怎么做」。AI 协作规则在 [CLAUDE.md](../CLAUDE.md)。

---

## 一句话原理

macOS 用「**这个 App 到底是谁**」来决定它能不能读钥匙串里的 key、能不能模拟键盘（辅助功能注入）。而「是谁」= **代码签名**。
**签名一变，系统就当成另一个陌生 App，之前给的所有授权（钥匙串、辅助功能）全部失效。**

记住这一条，下面所有坑都能推出来。

---

## 1. 为什么权限会反复要你授权（我们踩了一路的坑）

### 关键概念：designated requirement (DR)
每个签名的 App 都有一条「**身份指纹**」叫 designated requirement，长这样：

```
identifier "com.aubergine.audie" and anchor apple generic
  and certificate leaf[subject.CN] = "Apple Development: 你@xx (XXXX)"
```

= **bundle id**（`com.aubergine.audie`）+ **用哪张证书签的**。系统把「钥匙串始终允许」「辅助功能已授权」这些**钉在这条 DR 上**。下次某个 App 来读 key / 模拟键盘，系统比对它的 DR：对得上 → 放行；对不上 → 当陌生 App，重新弹窗要授权。

### ad-hoc 签名 = 每次都是「新 App」
`pnpm tauri dev` 跑的 `target/debug/audie`，默认是 **ad-hoc 签名**（临时签名）。它的 identifier 是 `audie-<一串hash>`，**hash 跟二进制内容走，每次重新编译都变**。

于是：每次改代码重编 → 二进制变 → hash 变 → DR 变 → 系统当成新 App → **钥匙串反复弹密码、辅助功能反复失效**。这就是你一直被烦的根。

```
ad-hoc：  audie-ef35140ff0ae4791   （下次编译 → audie-9a3f...   → 又变）
稳定签名：com.aubergine.audie      （永远不变 ← 我们要的）
```

### 两样东西都绑签名，不只是钥匙串
| 系统能力 | 绑什么 | 签名变了会怎样 |
|---|---|---|
| 钥匙串「始终允许」(ACL) | DR | 反复弹登录密码 |
| 辅助功能 / 模拟键盘 (TCC) | DR | 注入静默失败（事件被丢，还误报成功） |
| 麦克风 (TCC) | DR | 一般会自动重新弹「允许」 |

> 顺带一个魔鬼细节：辅助功能失效时，`CGEvent::post()` 是无返回值的，事件被系统**静默丢弃也不报错**，而 `CGPreflightPostEventAccess()` 还会返回**过期的 true**——所以代码层面根本测不出来，只会「显示成功但没字」。这就是为什么我们加了「注入文本始终留剪贴板」兜底（见 commit `a65fddf`）。

---

## 2. 开发期我们是怎么解决的（已落地，commit `4823a3c`）

核心：**让 dev 二进制每次都用同一张证书重签**，DR 就稳了。

```
pnpm tauri dev  →  cargo run  →  读 .cargo/config.toml 里的 runner
                                  ↓
                      scripts/dev-sign-run.sh  （跑二进制前先 codesign 再 exec）
                                  ↓
        codesign --force -i com.aubergine.audie --sign <你的证书>
```

- **`-i com.aubergine.audie` 是命门**：把 DR 的 identifier 那半也钉成固定 bundle id（不然默认用每次变的 `audie-<hash>`，DR 照样漂）。
- **证书身份**从 `$AUDIE_SIGN_IDENTITY` 或 gitignored 的 `.cargo/sign-identity` 读。没证书的贡献者：脚本 fall through 跑未签名，不被卡。
- ⚠️ **`.cargo/sign-identity` 是本机文件、不入库**。换机器 / fresh clone / 误删它 → 又跑未签名 → 弹窗失焦全回来。重建它（填那串 hash）或 `export AUDIE_SIGN_IDENTITY=...` 即可。

### 切到稳定签名 = 要重新授权一次（一次性迁移）
因为签名从 ad-hoc 换成了固定证书 = **换了身份**，所有旧授权失效，要对**新身份**重授一次。之后因为签名稳定，跨重编永久有效：

1. **钥匙串**：第一次读 key 弹一次 → 点「**始终允许**」（不是「允许」）。
2. **辅助功能**：系统设置 → 隐私与安全性 → 辅助功能 → 手动加 `audie`（裸二进制点 `+`，`⌘⇧G` 粘路径 `src-tauri/target/debug/audie`）→ 打开开关。
   - Apple **不允许命令行授予**辅助功能（`tccutil` 只能删不能授），所以这步必须 GUI。
   - 授权对**已运行的进程不生效**，授完要**重启 `pnpm tauri dev`**。

### 自检命令
```bash
# 看二进制签名身份（应为 com.aubergine.audie + Team ID，flags 无 adhoc）
codesign -dv --verbose=4 src-tauri/target/debug/audie

# 看 DR（重编后再跑一次，两次应一致 → 这就是授权能持续的原因）
codesign -d --requirements - src-tauri/target/debug/audie
```

---

## 3. 发版（放 GitHub Release）该怎么做 —— 你要学的

> ⚠️ **开发签名 ≠ 发布签名。** 上面那张 **Apple Development** 证书只能在**你自己注册的机器**上跑，别人下载你的 app 用不了。发版要换一套。

### 发布三件套（缺一不可）
| # | 要做 | 没做会怎样 |
|---|---|---|
| 1 | 用 **Developer ID Application** 证书签名 | 别人 Mac 上 Gatekeeper 直接拦 |
| 2 | 开 **Hardened Runtime**（强化运行时） | 不开就没法公证 |
| 3 | **Notarization（公证）**：上传给 Apple 扫毒 → 拿回票据 → staple 钉进 app | 用户打开报「已损坏 / 无法验证开发者」 |

- **Developer ID Application**：专门给「**不走 App Store、直接分发**」的 app 用（GitHub Release 正是这种）。在 [developer.apple.com](https://developer.apple.com) → Certificates 里创建。需要 **Apple Developer Program 付费会员（$99/年）**——你已经有 Team ID（`CL54DSKC64`），大概率已经是会员，直接建证书就行。
- **公证**：Apple 自动扫描你的 app 有没有恶意代码，通过后给一张票据。`staple` 是把票据钉进 app，这样用户**离线**也能验证。**不公证 = 普通用户根本打不开**，这是 macOS 的硬门槛。

### Tauri 里怎么配
`tauri build`（打包 .app/.dmg）读 `tauri.conf.json` 的 `bundle.macOS`：

```jsonc
"bundle": {
  "macOS": {
    "signingIdentity": "Developer ID Application: 你的名字 (CL54DSKC64)",
    "hardenedRuntime": true,
    "entitlements": "Entitlements.plist",     // 麦克风等权限的 entitlements
    "minimumSystemVersion": "11.0"
  }
}
```

公证靠**环境变量**（Tauri bundler 自动调用）：
```bash
APPLE_ID="你的apple id邮箱"
APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # App 专用密码，不是登录密码！在 appleid.apple.com 生成
APPLE_TEAM_ID="CL54DSKC64"
# 然后 pnpm tauri build → 自动签名 + 公证 + staple
```
> 这些**绝不写进仓库**。本地放 shell env，CI 放 GitHub Secrets。

### 自动化：GitHub Actions + tauri-action（推荐，对应项目 P4）
官方 [`tauri-action`](https://github.com/tauri-apps/tauri-action)：**打个 git tag → 自动构建 + 签名 + 公证 + 建 GitHub Release 挂上 .dmg**。你要往 GitHub Secrets 里放：

| Secret | 是什么 |
|---|---|
| `APPLE_CERTIFICATE` | Developer ID 证书导出的 .p12，base64 编码 |
| `APPLE_CERTIFICATE_PASSWORD` | 导出 .p12 时设的密码 |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: ... (CL54DSKC64)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | 公证用（上面那仨） |
| `TAURI_SIGNING_PRIVATE_KEY` (+ password) | 见下，自动更新用，跟 Apple 签名是两套 |

### 别混：Apple 签名 ≠ Tauri 自动更新签名
两套**完全不同**的签名，新手常搞混：

| 签名 | 解决什么 | 工具 |
|---|---|---|
| Apple 代码签名 + 公证 | **用户能不能打开你的 app**（Gatekeeper） | Apple 证书 / `codesign` / `notarytool` |
| Tauri updater 签名 | **app 怎么确认一个自动更新包是你发的、没被掉包** | `minisign`，公钥进 `tauri.conf.json` 的 `plugins.updater.pubkey`，私钥进 Secrets |

发版要两套都配。Apple 那套保证「能装」，minisign 那套保证「自动更新可信」。

### 发版一步步清单（等你真要发时照着做）
1. [ ] 确认 Apple Developer Program 会员有效（Team ID `CL54DSKC64`）。
2. [ ] 在 developer.apple.com 建 **Developer ID Application** 证书，下到本机钥匙串。
3. [ ] `tauri.conf.json` 填 `bundle.macOS`（signingIdentity / hardenedRuntime / entitlements）。
4. [ ] appleid.apple.com 生成 **App 专用密码**，配公证 env，本地 `pnpm tauri build` 跑通一次（手动验证别人机器能打开）。
5. [ ] 生成 Tauri updater minisign 密钥对，公钥进 conf、私钥进 Secrets。
6. [ ] 写 `.github/workflows/release.yml` 用 `tauri-action`，tag 触发。
7. [ ] 证书 .p12 / 各密码 / minisign 私钥 全进 **GitHub Secrets**。
8. [ ] 打个 `v0.1.0` tag → push → 看 Actions 自动出 Release。

---

## 名词速查
| 词 | 人话 |
|---|---|
| 代码签名 (codesign) | 给 app 盖个「这是谁做的」的章 |
| designated requirement (DR) | 那个章的「身份指纹」，系统拿它认 app |
| ad-hoc 签名 | 临时章，每次编译都变，等于没固定身份 |
| TCC | macOS 管「麦克风/辅助功能/屏幕录制」这些隐私授权的系统 |
| Gatekeeper | 用户打开 app 时的「这 app 可信吗」门卫 |
| Notarization 公证 | 把 app 交 Apple 扫毒、拿通行证，过了门卫才放行 |
| Hardened Runtime | 一套运行时安全限制，公证的前提 |
| Apple Development 证书 | 只给**自己机器**开发用（我们 dev 在用的） |
| Developer ID 证书 | 给**对外分发**用（发版要的） |
| minisign | Tauri 自动更新用的签名，跟 Apple 签名无关 |
