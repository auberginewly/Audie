# Audie · 版本号命名规则（SemVer）

> 本项目的 SemVer 约定；实际发布入口与产物以 [release-workflow.md](specs/release-workflow.md) 和 [release.yml](../.github/workflows/release.yml) 为准。

## 一句话

**`MAJOR.MINOR.PATCH`**，例 `1.4.2` = 主版本.次版本.修订号。从左到右，越左改动越大。

## 三段含义 + 什么时候 +1

| 段             | 名字   | 什么时候 +1                                                       | 改动类型示例（非排期）            |
| -------------- | ------ | ----------------------------------------------------------------- | --------------------------------- |
| **MAJOR** 主   | 破坏性 | **不兼容**的改动：配置/数据格式不兼容、删功能、改了用户依赖的行为 | 配置文件结构大改、注入方式换掉    |
| **MINOR** 次   | 功能   | **向下兼容**的新功能                                              | 自动分点、自定义词表、新 provider |
| **PATCH** 修订 | 修复   | **向下兼容**的 bug 修复                                           | 注入失焦修复、流式收尾修复        |

规则：

- 每段都是**非负整数、禁止补零**（`1.01.0` ❌）。
- 高位 +1，低位**归零**：`1.4.2` 加功能 → `1.5.0`；再修 bug → `1.5.1`。
- **发布后任何改动都要发新版本号**，发出去的版本永不覆盖。
- 比较大小：依次比 MAJOR→MINOR→PATCH。

## 映射到本项目的 commit（Conventional Commits）

以下是人工选择版本时的判断依据，不是按 commit 自动计算版本的机制：

- `feat:` → 升 **MINOR**
- `fix:` → 升 **PATCH**
- 破坏性（commit 标 `feat!:` 或 footer 写 `BREAKING CHANGE:`）→ 升 **MAJOR**
- `docs:`/`chore:`/`refactor:` → 不单独发版，攒着随下个 feat/fix 一起出。

## SemVer 预发布后缀与 GitHub Pre-release

SemVer 支持预发布后缀，**优先级低于无后缀版本**（以下仅解释排序）：

- `alpha` 内部版 → `beta` 公测版 → `rc`(Release Candidate) 正式候选
- 格式 + 排序：`1.0.0-alpha.0` < `1.0.0-beta.0` < `1.0.0-rc.0` < `1.0.0`

当前发布校验脚本只接受无后缀的 `MAJOR.MINOR.PATCH`；上面的 `alpha/beta/rc` 示例不代表当前工作流可直接发布。GitHub 的 Pre-release 是独立标记：当前工作流即使版本无后缀，也会使用 `--prerelease`。历史 [v0.2.1](https://github.com/auberginewly/Audie/releases/tag/v0.2.1) 已发布且该标记为 false，不能据此推断未来发布标记。

## 0.x 特殊约定（本项目现在就在这）

当前三处版本均为 **`0.2.1`**，下一版本号尚未决定。**1.0 之前一切都可能变**，所以：

- 破坏性改动也**只升 MINOR**（`0.1.0` → `0.2.0`），不强行升到 1.0。
- `1.0.0` 是一个**公开承诺：稳定了**——核心听写闭环定型、配置格式不再乱改，才发 1.0.0。别过早 1.0。

## 版本号在本项目放哪（三处保持一致）

- `src-tauri/tauri.conf.json` 的 `version` ← 应用元数据与打包使用；updater 尚未实现
- `src-tauri/Cargo.toml` 的 `version`
- `package.json` 的 `version`
- 合并 `main -> release` 后，`release` 分支 push 触发 GitHub Actions，构建验证通过后创建带 `v` 前缀的 tag 和 GitHub Pre-release。发版说明来自根目录 `RELEASE_NOTES.md`。
- 运行 `node scripts/check-release-version.mjs` 检查三处版本与双语说明。发布前先同步远端 tags，再运行 `node scripts/check-release-version.mjs --require-unreleased`；该选项检查本地 tag 是否已存在，不能代替同步远端状态。
- `v0.2.1` 已存在，原版本不能再次通过未发布校验。本地重打 `0.2.1` 安装包仅用于验证，不覆盖既有发布。

## 版本选择示例（仅假设，不是路线图）

| 假设改动             | 假设版本变化      |
| -------------------- | ----------------- |
| 兼容的 bug 修复      | `0.2.1` → `0.2.2` |
| 兼容的新功能         | `0.2.1` → `0.3.0` |
| 0.x 阶段的不兼容改动 | `0.2.1` → `0.3.0` |

实际下一版由维护者根据最终范围决定。自动分点、自定义术语等仍是未选定的未来候选，不与任何版本号绑定。

## 常见误区

- 补零（`1.01.0`）❌
- 发布后偷偷改内容、不升版本号 ❌
- 拿 PATCH 塞新功能 ❌（新功能是 MINOR）
- 0.x 阶段就纠结 MAJOR ❌（1.0 前用 MINOR 表达破坏性就行）
