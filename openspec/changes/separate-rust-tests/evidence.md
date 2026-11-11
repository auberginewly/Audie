# 仓库整理第一片：测试分离 · 2026-11-11

本片承接总体整理任务，不是恢复功能修改；基线为已合并恢复修复的 main 35a7dbb。后续设置、pipeline 与前端功能收拢尚未实施。

## 已落实的文件组织

| 原文件 | 搬迁前 | 搬迁后 | 测试归属 |
| --- | --- | --- | --- |
| src-tauri/src/lib.rs | 2054 行 | 1667 行 | tests/lib_tests.rs + lib/{asr_config,enhance_config,take_and_stream}.rs |
| src-tauri/src/commands.rs | 1595 行 | 1014 行 | tests/commands_tests.rs + commands/{settings,providers,hotkeys,secrets}.rs |

37 个原测试函数（含 Windows cfg 的两个函数）均保留原属性和正文，生产正文逐字不变；只将 cfg(test) 内联模块替换为带显式 path 的外部单元测试声明。没有扩大可见性、增加依赖或修改业务。

新入口/分组文件分别为 10、13、56、69、117、139、186、203、203 行，全部少于 400 行。既有 tests/recovery/ 目录没有改动。

## 本轮验证

- 搬迁前后 Cargo --list 的 201 个用例身份逐个一致（忽略新增模块路径前缀，不只是比较总数）。
- 对照提取前的完整源码及每个测试函数的去缩进正文，生产前后缀、测试属性、断言和函数身份均一致；包括本机未运行的 Windows 条件用例。
- cargo test：200 passed / 1 ignored；bin/doc tests 均通过。
- cargo fmt --check、cargo clippy -- -D warnings 通过；Cargo 的既有 block 0.1.6 未来兼容性提示仍在。
- pnpm test：26 passed；typecheck、lint（0 error / 34 条既有 warning）、format:check、build 通过。
- pnpm tauri build --debug --no-bundle 通过。
- 新文件生产可达性与 cfg(test) 边界由 native 非测试构建验证；没有新增“证明文件名”的脆弱目录测试。

原源码、HEAD、用例及逐函数比对结果存于 `/var/folders/nj/h5ghbg792f3cqjjhml4z73p40000gn/T/audie-test-layout-b5kkqlxb/`，本轮命令输出存于 `/tmp/audie-test-layout-*.log`。

## 进度边界

本片完成测试物理分离，不表示 lib.rs/commands.rs 的业务职责已拆完，它们仍超过 400 行。下一片是 settings 规则与 command 入口归属。

没有执行新的真实麦克风、三模式录音和跨 App 粘贴走查；恢复切片的手动待验项不因本片变绿。父计划中相关手动验收仍未勾选。
