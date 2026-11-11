# Design

## Context

本片承接已批准整体方案；作者已授权继续所有剩余组织调整。分工只限能力模块，核心装配和设置模块由主任务维护，前端由另一执行单元维护。

## Goals / Non-Goals

代码按既有职责归属，保留公共入口、实际执行顺序、锁/线程/资源生命周期、SQL、网络请求及所有测试。无新能力、依赖、Manager 或抽象；不借整理修复 provider 协议。

## Decisions

- audio/mod.rs 保留 Manager 和共享私有会话类型；recording 实现录音 API，capture 承接原 CPAL 线程和设备枚举，monitor 承接预览和事件 emitter，buffer 承接原样样本处理/保险丝/队列方法。共享结构仍为私有，兄弟间入口仅 pub(super)。
- history/mod.rs 保留 Manager API 和数据合同；storage 为建表/已有兼容 ALTER，queries 为条目 SQL，stats 为聚合 SQL。公开方法、字段、数据目录和 SQL 不变。
- llm/mod.rs 保留 trait、build_provider、local 判定，openai_compatible 为请求/错误映射，sanitize 为输出清理。既有对外类型和辅助函数路径由有目的的 reexport 保留。
- provider_test/mod.rs 保留请求/返回合同和共用鉴权/HTTP 错误映射；connection/models 保存各自原命令，入口由主任务更新为完整注册路径。
- ASR 小文件独立测试后保持原职责；仅豆包 client 仍超长，将连接配置、鉴权头归既有 config，PCM 编码归既有 codec，client 对原配置类型路径保留 reexport；不合并或重写 PCM 算法。
- 测试在所属目录单独文件，按生产模块私有作用域挂载；不为测试扩大业务符号权限。校验测试函数身份、属性及正文，Cargo 发现允许模块前缀变化。

## Risks / Trade-offs

路径变化可能漏注册命令或测试，由 Cargo 测试清单、编译和主任务原生构建验证；真实麦克风、provider 联网与 Windows 系统行为不以静态检查代替。

## Migration Plan

先保存原 owned 源码；机械分离测试和职责并核对原函数/测试正文；与核心集成后运行 Rust 自动检查，记录实际证据。由主任务负责状态文档与整体验收。
