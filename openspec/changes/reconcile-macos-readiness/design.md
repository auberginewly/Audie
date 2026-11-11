# Design

## 文件与职责

- docs/STATUS.md：只改历史附录前的现状，分开已实现、实际已验收、未覆盖与已免除；历史轨迹原样保留并明确是历史记录。
- docs/ROADMAP.md：本机私有文件，修复发布入口与小版本口径，保持候选功能和作者排期权；同步主工作区本机副本，不强制加入 Git。
- docs/VERSIONING.md：当前 0.2.1 与 SemVer 示例分开，版本来源以三份清单为准。
- docs/specs/release-workflow.md：反映 release.yml 现有双平台构建、验证与预发布行为；已发布版本不覆盖，安装/升级验证区分原生包与开发程序。
- 本 change 的 readiness-evidence.md：记录基线、文档链接检查、打包/安装副本与用户配合的实际证据。

## 验证边界

用户批准的是现有功能收尾。打包使用现有 CI 的 Apple Silicon target 与 signingIdentity=-，产物是当前 0.2.1 的本地验证包；不发布同版本的新产物。挂载只读 DMG并复制到临时目录，验证与重启均使用受控路径，不覆盖已安装包。若新 bundle 与 debug 的 TCC/Keychain 身份不同，真实授权和密钥配置由作者在应用中完成，不以开发程序验收冒充发布包验收。

基线为 d8ce285，main/worktree 干净，上一轮双平台 CI 42982804213 success。本轮保留已验证普通听写/录音中取消撤销结果，追加真实 LLM 调用/目标结果证据；用户已回答有可用 LLM，当前 compose_hotkey 为空，需要在应用里配置独立触发键后才能测写作。
