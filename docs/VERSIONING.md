# Audie · 版本号命名规则（SemVer）

> 总结自[掘金《前端版本号管理》](https://juejin.cn/post/7554702802972459047) + [semver.org](https://semver.org)，落到本项目（Tauri app，git tag 发版）。

## 一句话

**`MAJOR.MINOR.PATCH`**，例 `1.4.2` = 主版本.次版本.修订号。从左到右，越左改动越大。

## 三段含义 + 什么时候 +1

| 段 | 名字 | 什么时候 +1 | 例（本项目） |
|---|---|---|---|
| **MAJOR** 主 | 破坏性 | **不兼容**的改动：配置/数据格式不兼容、删功能、改了用户依赖的行为 | 配置文件结构大改、注入方式换掉 |
| **MINOR** 次 | 功能 | **向下兼容**的新功能 | 自动分点、自定义词表、新 provider |
| **PATCH** 修订 | 修复 | **向下兼容**的 bug 修复 | 注入失焦修复、流式收尾修复 |

规则：
- 每段都是**非负整数、禁止补零**（`1.01.0` ❌）。
- 高位 +1，低位**归零**：`1.4.2` 加功能 → `1.5.0`；再修 bug → `1.5.1`。
- **发布后任何改动都要发新版本号**，发出去的版本永不覆盖。
- 比较大小：依次比 MAJOR→MINOR→PATCH。

## 映射到本项目的 commit（Conventional Commits）

本项目 commit 已用 `feat:`/`fix:`/`docs:`…，正好对上：
- `feat:` → 升 **MINOR**
- `fix:` → 升 **PATCH**
- 破坏性（commit 标 `feat!:` 或 footer 写 `BREAKING CHANGE:`）→ 升 **MAJOR**
- `docs:`/`chore:`/`refactor:` → 不单独发版，攒着随下个 feat/fix 一起出。

## 预发布版本（功能没稳前先放出去）

发大版本但没完全稳时用，**优先级低于正式版**：
- `alpha` 内部版 → `beta` 公测版 → `rc`(Release Candidate) 正式候选
- 格式 + 排序：`1.0.0-alpha.0` < `1.0.0-beta.0` < `1.0.0-rc.0` < `1.0.0`

## 0.x 特殊约定（本项目现在就在这）

当前 `0.0.0`。**1.0 之前一切都可能变**，所以：
- 破坏性改动也**只升 MINOR**（`0.1.0` → `0.2.0`），不强行升到 1.0。
- `1.0.0` 是一个**公开承诺：稳定了**——核心听写闭环定型、配置格式不再乱改，才发 1.0.0。别过早 1.0。

## 版本号在本项目放哪（三处保持一致）

- `src-tauri/tauri.conf.json` 的 `version` ← 打包 / updater 认这个
- `src-tauri/Cargo.toml` 的 `version`
- `package.json` 的 `version`
- 合并 `main -> release` 后，GitHub Actions 根据三处一致的版本自动创建带 `v` 前缀的 tag 和 GitHub Release。发版说明来自根目录 `RELEASE_NOTES.md`。
- 发布前运行 `node scripts/check-release-version.mjs --require-unreleased`，确认版本、双语说明和 tag 都满足发布条件。

## 建议的发版节奏（给本项目）

| 版本 | 触发 |
|---|---|
| `0.1.0` | **现在**——核心 done，发第一个能用的版本 |
| `0.2.0` | 加完自动分点（MINOR 新功能） |
| `0.2.1` | 修个 bug（PATCH） |
| `0.2.0-beta.0` | 想公开拉人公测、但没完全稳 |
| `1.0.0` | 产品定型、配置稳定、敢说「稳了」 |

## 常见误区

- 补零（`1.01.0`）❌
- 发布后偷偷改内容、不升版本号 ❌
- 拿 PATCH 塞新功能 ❌（新功能是 MINOR）
- 0.x 阶段就纠结 MAJOR ❌（1.0 前用 MINOR 表达破坏性就行）
