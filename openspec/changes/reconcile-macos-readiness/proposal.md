# Proposal

## Why

作者已批准优先完成 macOS 核心模式验收、文档残留清理和现有安装包验证。当前 STATUS 的现状段仍有“未 commit / 未编译”、已退役 Groq、已免除 toast 逐态验收与尚无分发的过时说法；本机 ROADMAP 引用不存在的旧发布方案；版本/发布流程仍按旧 0.1 描述。

## Scope

核对源码、Git 与真实 GitHub CI/Release，更新现状和现行发布流程，保留历史附录。沿用现有 Tauri/macOS ad-hoc 打包命令产出本地 DMG，验证挂载、架构、版本、签名与隔离目录安装副本；不覆盖 /Applications/Audie.app、不重置 TCC 或读取密钥。已有 LLM 的核心模式由作者配合口述，在受控目标中独立核对；不能覆盖的系统场景明确保留。

## Out of Scope

不做新功能、自动分点/术语表、agent、新库或架构层；不变更版本号、不创建 tag/Release、不合并 release 分支、不重建历史方案内容。维护者本机 ROADMAP 保持既有私有/忽略状态。
