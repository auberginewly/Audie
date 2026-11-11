import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import vm from "node:vm";
const evidence = "openspec/changes/organize-frontend-features/evidence/";
const testSource = readFileSync("src/features/settings/__tests__/settings-schema.test.mjs", "utf8");
const settings = vm.runInNewContext("(" + testSource.match(/const baseSettings = (\{[\s\S]*?\n\});/)[1] + ")");
settings.onboarding_completed = true;
const mock = `(() => {
 const callbacks = new Map(), listeners = new Map(); let next=0;
 window.__smokeSettings=${JSON.stringify(settings)}; window.__smokeCommands=[];
 window.__smokeEmit=(event,payload)=>{for(const entry of listeners.values())if(entry.event===event) callbacks.get(entry.handler)?.({event,payload,id:0});};
 window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:(_event,id)=>listeners.delete(id)};
 window.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},
 transformCallback:(fn)=>{callbacks.set(++next,fn);return next;},unregisterCallback:(id)=>callbacks.delete(id),
 invoke: async (cmd,args={}) => {
 window.__smokeCommands.push({cmd,args});
 if(cmd==='plugin:event|listen'){const id=++next;listeners.set(id,args);return id;}
 if(cmd==='plugin:event|unlisten')return;
 if(cmd==='plugin:app|version')return '0.2.1';
 if(cmd==='get_settings')return window.__smokeSettings;
 if(cmd==='update_settings'){Object.assign(window.__smokeSettings,args.patch);return window.__smokeSettings;}
 if(cmd==='has_secret')return true;
 if(cmd.includes('permission_status')||cmd==='get_input_monitoring_status')return true;
 if(cmd==='auto_input_device')return 'Built-in Microphone';
 if(cmd==='list_microphones')return [{id:'Built-in Microphone',label:'Built-in Microphone'}];
 if(cmd==='get_usage_stats')return {spoken_words:100,spoken_duration_ms:60000,spoken_count:1,ai_output_words:40};
 if(['get_daily_usage','list_history','list_asr_providers','list_llm_providers','discover_local_llm'].includes(cmd))return [];
 if(cmd==='plugin:autostart|is_enabled')return false;
 return null;
 }};
})()`;
const profile = mkdtempSync(join(tmpdir(), "audie-chrome-"));
const server = spawn(
  process.execPath,
  ["node_modules/vite/bin/vite.js", "preview", "--host", "127.0.0.1", "--port", "1422"],
  { stdio: "ignore" },
);
const chrome = spawn(
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  [
    "--headless=new",
    "--disable-gpu",
    "--no-first-run",
    "--no-default-browser-check",
    "--remote-debugging-port=9223",
    `--user-data-dir=${profile}`,
    "about:blank",
  ],
  { stdio: "ignore" },
);
const delay = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(fn, description) {
  for (let i = 0; i < 150; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await delay(50);
  }
  throw Error("Timed out: " + description);
}
let ws;
const results = [];
const errors = [];
try {
  await until(async () => (await fetch("http://127.0.0.1:1422")).ok, "preview server");
  const target = await until(
    async () =>
      await (async () => {
        const response = await fetch("http://127.0.0.1:9223/json/new?about:blank", { method: "PUT" });
        if (!response.ok) throw new Error(`Chrome target HTTP ${response.status}`);
        return response.json();
      })(),
    "Chrome target",
  );
  ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", ({ data }) => {
    const m = JSON.parse(data);
    if (m.id) {
      const f = pending.get(m.id);
      pending.delete(m.id);
      m.error ? f.reject(Error(JSON.stringify(m.error))) : f.resolve(m.result);
    }
    if (m.method === "Runtime.exceptionThrown") errors.push(m.params.exceptionDetails);
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      pending.set(++id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  const text = () => evaluate("document.body.innerText");
  const expect = async (name, expression) => {
    await until(() => evaluate(expression), name);
    results.push(name);
  };
  const click = async (label) =>
    evaluate(
      `(() => { const button=[...(document.querySelector('[role=dialog]')??document).querySelectorAll('button')].find(b=>b.textContent.trim()===${JSON.stringify(label)}||b.textContent.trim().startsWith(${JSON.stringify(label)})||b.getAttribute('aria-label')===${JSON.stringify(label)});if(!button)throw Error('missing button '+${JSON.stringify(label)});button.click();return true;})()`,
    );
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Page.addScriptToEvaluateOnNewDocument", { source: mock });
  await send("Page.navigate", { url: "http://127.0.0.1:1422/" });
  await expect(
    "main home mounts with settings and packaged version",
    `document.body.innerText.includes('言为心声')&&document.body.innerText.includes('0.2.1')`,
  );
  await click("历史记录");
  await expect("history empty state mounts", `document.body.innerText.includes('还没有历史记录')`);
  await click("设置");
  await expect(
    "model settings mounts",
    `document.querySelector('[role=dialog]')&&document.body.innerText.includes('文本处理')`,
  );
  await click("文本处理");
  await until(() => evaluate(`document.body.innerText.includes('clean up')`), "prompt preview");
  await evaluate(`[...document.querySelectorAll('button')].find(b=>b.textContent.includes('clean up')).click()`);
  await expect("polish prompt editor mounts", `document.querySelector('textarea')?.value==='clean up'`);
  await click("改写");
  await until(() => evaluate(`document.body.innerText.includes('rewrite')`), "rewrite preview");
  await evaluate(`[...document.querySelectorAll('button')].find(b=>b.textContent.includes('rewrite')).click()`);
  await expect("rewrite prompt editor mounts", `document.querySelector('textarea')?.value==='rewrite'`);
  await click("写作");
  await until(() => evaluate(`document.body.innerText.includes('compose')`), "compose preview");
  await evaluate(`[...document.querySelectorAll('button')].find(b=>b.textContent.includes('compose')).click()`);
  await expect("compose prompt editor mounts", `document.querySelector('textarea')?.value==='compose'`);
  await click("通用");
  await expect("general device picker mounts", `document.body.innerText.includes('Built-in Microphone')`);
  await click("关于");
  await expect("about page mounts", `document.body.innerText.includes('GitHub Issues')`);
  await click("运行");
  await expect(
    "rerun wizard opens from shared App coordinator",
    `window.__smokeSettings.onboarding_completed===false&&document.body.innerText.includes('欢迎使用 Audie')`,
  );
  await send("Page.captureScreenshot", { format: "png" }).then((r) =>
    writeFileSync(evidence + "main-browser-smoke.png", Buffer.from(r.data, "base64")),
  );
  await send("Page.navigate", { url: "http://127.0.0.1:1422/overlay.html" });
  await expect(
    "overlay registers state listener",
    `window.__smokeCommands?.some(c=>c.cmd==='plugin:event|listen'&&c.args.event==='state-change')`,
  );
  await evaluate(`window.__smokeEmit('state-change',{from:'IDLE',to:'RECORDING'})`);
  await expect(
    "recording waveform and controls mount",
    `document.querySelectorAll('button').length===2&&document.body.innerText.includes('0:00')`,
  );
  await evaluate(`window.__smokeEmit('state-change',{from:'RECORDING',to:'PROCESSING'})`);
  await expect("processing capsule mounts", `document.body.innerText.includes('转写中')`);
  await evaluate(`window.__smokeEmit('enhance-progress',{phase:'started',message:'polishing'})`);
  await expect("polishing capsule mounts", `document.body.innerText.includes('润色中')`);
  await evaluate(`window.__smokeEmit('state-change',{from:'PROCESSING',to:'SUCCESS'})`);
  await expect("success capsule mounts", `document.body.innerText.includes('已插入')`);
  await evaluate(`window.__smokeEmit('state-change',{from:'SUCCESS',to:'CANCEL'})`);
  await expect(
    "cancel toast and undo action mount",
    `document.body.innerText.includes('已取消')&&document.body.innerText.includes('撤销')`,
  );
  await click("撤销");
  await expect("undo action preserves command contract", `window.__smokeCommands.some(c=>c.cmd==='undo_last')`);
  await evaluate(
    `window.__smokeEmit('error',{code:'network',message:'smoke network error',recoverable:true});window.__smokeEmit('state-change',{from:'CANCEL',to:'ERROR'})`,
  );
  await expect(
    "error toast and retry action mount",
    `document.body.innerText.includes('smoke network error')&&document.body.innerText.includes('重试')`,
  );
  await click("重试");
  await expect("retry action preserves command contract", `window.__smokeCommands.some(c=>c.cmd==='retry_last')`);
  await send("Page.captureScreenshot", { format: "png" }).then((r) =>
    writeFileSync(evidence + "overlay-browser-smoke.png", Buffer.from(r.data, "base64")),
  );
  if (errors.length) throw Error("Browser runtime exceptions: " + JSON.stringify(errors));
  console.log(JSON.stringify({ passed: results.length, results, errors }, null, 2));
  writeFileSync(
    evidence + "browser-smoke.json",
    JSON.stringify(
      {
        method:
          "Headless Chrome CDP, production Vite preview, mocked Tauri IPC; no native OS or real provider validation",
        passed: results.length,
        results,
        errors,
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  ws?.close();
  chrome.kill();
  server.kill();
  await delay(300);
  rmSync(profile, { recursive: true, force: true });
}
