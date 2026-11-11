import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

import { zhHans } from "../../../i18n/locales/zh-Hans.ts";

const require = createRequire(import.meta.url);

// 只替换 IPC、store 和波形边界；实际胶囊及 toast 组件参与渲染，
// 避免仅检查源码字符串而漏掉标题/副标题接错的问题。
function renderFailure(message) {
  const state = { state: "SUCCESS", error: null, enhanceProgress: { phase: "failed", message } };
  const mocks = {
    "./recording": { useRecordingStore: (select) => select(state) },
    "./Waveform": { useAudioLevels: () => [], Waveform: () => null },
    "./useAudioLevels": { useAudioLevels: () => [] },
    "./overlayCommands": { callOverlayCommand: () => {} },
    "../../components/ui": { Icon: () => null },
    "../../i18n": { useI18n: () => ({ t: (key) => zhHans[key] }) },
  };
  function load(name) {
    const filename = new URL(`../${name}.tsx`, import.meta.url);
    const code = ts.transpileModule(readFileSync(filename, "utf8"), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
    }).outputText;
    const module = { exports: {} };
    const resolve = (id) => mocks[id] ?? (id === "./CapsuleToast" ? load("CapsuleToast") : require(id));
    new Function("require", "module", "exports", code)(resolve, module, module.exports);
    return module.exports;
  }
  return renderToStaticMarkup(createElement(load("Capsule").Capsule));
}

for (const reason of ["API key 无效，请检查设置", "请求过于频繁或额度受限", "网络连接失败"]) {
  test(`fallback toast preserves the actual reason: ${reason}`, () => {
    const html = renderFailure(reason);
    assert.ok(html.includes("已插入原文"));
    assert.ok(html.includes(reason));
    assert.ok(!html.includes("未配置润色模型"));
  });
}

test("an old failed event without a reason uses a generic fallback", () => {
  const html = renderFailure("");
  assert.ok(html.includes("已插入原文"));
  assert.ok(html.includes("润色失败，请检查模型连接设置"));
  assert.ok(!html.includes("未配置润色模型"));
});
