import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";
import { setImmediate } from "node:timers";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import * as dictationEvents from "../../features/dictation/events.ts";

const sourceRoot = fileURLToPath(new URL("../../", import.meta.url));
function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? sourceFiles(path) : /\.tsx?$/.test(path) ? [path] : [];
  });
}

test("frontend diagnostics never evaluate or log private payloads, Zod errors or unknown exceptions", () => {
  const marker = "PRIVATE_KEY_TRANSCRIPT_PROMPT_SELECTION";
  let checked = 0;
  for (const path of sourceFiles(sourceRoot)) {
    const source = ts.createSourceFile(path, readFileSync(path, "utf8"), ts.ScriptTarget.Latest, true);
    function visit(node) {
      if (
        ts.isCallExpression(node) &&
        ts.isPropertyAccessExpression(node.expression) &&
        node.expression.expression.getText(source) === "console"
      ) {
        const captured = [];
        const console = Object.fromEntries(
          ["warn", "error", "log", "info", "debug"].map((level) => [level, (...args) => captured.push(args)]),
        );
        // 所有不可信来源放入相同 marker；实际执行日志语句，防止原始对象再次进入控制台。
        vm.runInNewContext(node.getText(source), {
          console,
          err: marker,
          error: marker,
          e2: marker,
          parsed: { error: { message: marker } },
          event: { payload: marker },
          key: "permission",
          cmd: "cancel",
        });
        assert.equal(JSON.stringify(captured).includes(marker), false, path);
        assert.equal(node.arguments.length, 1, `${path}: diagnostics need a fixed category only`);
        assert.equal(
          ts.isStringLiteral(node.arguments[0]),
          true,
          `${path}: category must not depend on external input`,
        );
        checked += 1;
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
  }
  assert.ok(checked > 20, "exercise the app's existing failure paths");
});

function recordingFlowHarness(rejectSubscriptions = false) {
  const callbacks = new Map();
  const logs = [];
  const writes = [];
  const removed = [];
  let cleanup;
  const store = Object.fromEntries(
    ["setState", "setError", "setEnhanceProgress"].map((name) => [name, (value) => writes.push({ name, value })]),
  );
  const dependencies = {
    react: {
      useEffect: (effect) => {
        cleanup = effect();
      },
    },
    "@tauri-apps/api/event": {
      listen: (name, callback) => {
        callbacks.set(name, callback);
        return rejectSubscriptions
          ? Promise.reject(new Error("PRIVATE_SUBSCRIPTION_ERROR"))
          : Promise.resolve(() => removed.push(name));
      },
    },
    "./events": dictationEvents,
    "./recording": { useRecordingStore: (selector) => selector(store) },
  };
  const source = readFileSync(new URL("../../features/dictation/useRecordingFlow.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } });
  const exports = {};
  vm.runInNewContext(outputText, {
    exports,
    require: (name) => {
      assert.ok(Object.hasOwn(dependencies, name), `unexpected dependency ${name}`);
      return dependencies[name];
    },
    console: { warn: (...args) => logs.push(args), error: (...args) => logs.push(args) },
  });
  exports.useRecordingFlow();
  return { callbacks, logs, writes, removed, cleanup: () => cleanup() };
}

test("malformed dictation events are rejected privately while valid state and cleanup still work", async () => {
  const flow = recordingFlowHarness();
  const marker = "PRIVATE_PAYLOAD_TRANSCRIPT_PROMPT_KEY";
  for (const callback of flow.callbacks.values())
    callback({ payload: { from: marker, code: marker, phase: marker, message: marker } });
  assert.equal(flow.writes.length, 0);
  assert.equal(flow.logs.length, 3);
  assert.equal(JSON.stringify(flow.logs).includes(marker), false);
  flow.callbacks.get("state-change")({ payload: { from: "IDLE", to: "RECORDING" } });
  flow.callbacks.get("error")({ payload: { code: "network", message: "safe failure", recoverable: true } });
  flow.callbacks.get("enhance-progress")({ payload: { phase: "started", message: "working" } });
  assert.deepEqual(
    flow.writes.map(({ name }) => name),
    ["setState", "setError", "setEnhanceProgress"],
  );
  assert.equal(flow.writes[0].value, "RECORDING");
  // 在订阅 promise 完成前卸载，迟到的 unlisten 仍须各执行一次。
  flow.cleanup();
  await new Promise(setImmediate);
  assert.deepEqual(flow.removed.sort(), ["enhance-progress", "error", "state-change"]);
});

test("subscription failures keep diagnostics without logging unknown errors", async () => {
  const flow = recordingFlowHarness(true);
  await new Promise(setImmediate);
  assert.equal(flow.logs.length, 3);
  assert.equal(JSON.stringify(flow.logs).includes("PRIVATE_SUBSCRIPTION_ERROR"), false);
  assert.equal(flow.writes.length, 0);
  flow.cleanup();
});
