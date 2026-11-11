# Proposal

## Why

作者要求修复原安装包权限问题。系统启动临时副本时，TCC 校验取到的是旧 `/Applications/Audie.app` 的签名摘要，与副本稳定签名不符；直接运行副本成功不能说明安装问题已修复。

## Scope

本机使用现有 Apple Development 身份构建完整 app，备份旧应用后安装到固定 canonical 路径，从 Launch Services 正常启动并核对授权、保存的设置及凭据。保留配置和系统 keychain，沿用现有权限 API。不发布新版本或修改 CI 发行签名，不添加库、平台抽象或权限绕过。
