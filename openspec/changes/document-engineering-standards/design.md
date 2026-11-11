# Design

## Decisions

公共规则分两份：CONTRIBUTING 管环境/命令/流程，ENGINEERING 管日常代码细则；职责与依赖继续归既有 ARCHITECTURE，进度与切片记录不堆入规范。

沿用当前命名、feature/Manager/Platform 与测试 runner，允许按用途的单向合同引用，不引入工具或新层。注释以中文为主，模块讲职责、接口讲约定、实现讲原因；系统代码写安全前提。开发优先 macOS，Windows/真实系统未测项如实记录。

AGENTS/CLAUDE 保留本机附加约束，仍被 Git 忽略；公共文档只链接版本化资源。本次明确逐文件加入 ENGINEERING，不改变整个 docs/ 的忽略策略。OpenSpec CLI 不作为应用运行依赖，也不假定 clone 自带维护者本机草案。

## Validation

仅运行 Markdown 格式、相对链接（公共入口同时核对 Git 跟踪）、命令/工作流/合同来源核对、OpenSpec strict validate 和 git diff --check；检查最终 diff 不含业务/依赖/CI 变更。不为纯文档重跑全量业务测试或声称系统演示通过。
