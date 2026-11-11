# Frontend organization evidence — 2026-11-11

Base: `5c38a85e3d9ee68c8a150db292d7082c9d58b591` (`main` with three original prompts). Worktree: `/Users/aubergine/.codex/worktrees/recovery-without-templates/Audie`, shared branch `codex/reorganize-modules`. Frontend scope only; Git index and commits remain coordinator-owned.

## Checks

| Check | Result / evidence |
| --- | --- |
| pnpm test | **PASS**, 7 files, 26 passed, 0 failed/skipped/cancelled — [test.log](evidence/test.log) |
| pnpm typecheck | **PASS**, exit 0 — [typecheck.log](evidence/typecheck.log) |
| pnpm lint | **PASS**, exit 0; 0 errors, 34 advisory warnings preserved — [lint.log](evidence/lint.log) |
| pnpm format:check | **PASS**, all matched TS/TSX/CSS/mjs formatted — [format-check.log](evidence/format-check.log) |
| pnpm build | **PASS**, both main and overlay production entry points — [build.log](evidence/build.log) |
| test:home-chart / test:settings-schema / test:onboarding-permissions | **PASS**, respectively 4 / 2 / 6 original cases |
| Runner negative checks | Empty fixture directory and intentional failing test each return exit **1** — [runner-edge-cases.json](evidence/runner-edge-cases.json) |
| Physical lines | No handwritten frontend source or test exceeds 400; largest ModelSection.tsx **374** — [line-counts.json](evidence/line-counts.json) |

## Behavior-preservation audits

- All **26 test identities** are identical, sorted before/after; no test removed or skipped. [Before](evidence/tests-before.json), [after](evidence/tests-after.json), [baseline run](evidence/tests-before.log).
- All **256 effective keys per language** (zh-Hans / zh-Hant / en; **768 key/value pairs**) are exactly equal after resolving zh-Hant's original fallback spread. [Before](evidence/translations-before.json), [after](evidence/translations-after.json). The original zh-Hans default, languages list and interpolation implementation are retained.
- AST string-literal multiset: **3457 → 3455**. The only removed literals are the type-only indexed access keys `asr_provider` / `llm_provider`, replaced by provider-owned inferred enum types. Every runtime string, translation, CSS class, inline style string, command name and event name is retained. [source-literal-audit.json](evidence/source-literal-audit.json).
- `git diff --name-only -- src/index.css src/styles` is empty. Dependencies and lockfile unchanged. Five startup requests remain the original same Promise.all, with the same parsing and failure handling.
- Runtime contracts live in their feature; provider request schema no longer imports SettingsSchema, so Settings → provider enum dependency has no runtime reverse edge. UI components may still use type-only Settings contracts. No multi-template or cycle behavior added.
- Source-based model test paths updated; its production-source search excludes `__tests__` now that its own prohibited-provider assertions are inside src. No assertions weakened.
- No React/Vite production import points at __tests__; tsconfig strict options are unchanged. Removed unused old aggregate entry files do not retain two implementations. Complete [migration map](evidence/migration-map.json).

## Running browser verification

**18 assertions passed; zero uncaught runtime exceptions**, using already installed headless Google Chrome over CDP, production `vite preview`, and explicit mocked Tauri IPC. Re-ran after driver status-check improvement. [Result](evidence/browser-smoke.json), [reproducible driver](evidence/browser-smoke.mjs). Run from repository root with `node openspec/changes/organize-frontend-features/evidence/browser-smoke.mjs` after pnpm build (requires installed Chrome at the recorded macOS path; ports 1422 / 9223).

Covered home metadata/version, history empty state, model settings, all three prompt disclosure/editors, microphone/general, About, rerun setup via shared App coordinator, overlay listener registration, recording waveform/buttons, transcribing/polishing/success/cancel/error displays, undo/retry dispatch with unchanged command names. [Main screenshot](evidence/main-browser-smoke.png), [overlay screenshot](evidence/overlay-browser-smoke.png), both visually inspected. Chrome's white transparency backdrop and in-progress overlay animation are browser capture artifacts; these are not native overlay fidelity evidence.

No Reticle tools were available. Browser mocks do **not** verify Rust commands, real recording, permission prompts, provider network calls, keychain, OS focus/injection, hotkey capture, multi-screen/Space behavior, or native overlay transparency. Those remain under the coordinator's native integration / manual matrix; no manual OS claim is made here.

## React Doctor

Applied react-doctor skill, tool **0.9.22**, installed only into npx cache; no project dependency or configuration added.

- Required `npx react-doctor@latest --verbose --diff`: **100/100**, but only **4 tracked changed files** were scanned because moves were untracked. This is explicitly insufficient by itself. [Log](evidence/react-doctor-diff.log).
- Proper `--verbose --scope full` comparison against a complete isolated `git archive` of the base with the same existing node_modules: baseline **50/100, 38 diagnostics** → current **52/100, 38 diagnostics**. The invalid preliminary `--scope all` attempt was discarded; the final logs use the tool-supported `full` scope. [Baseline](evidence/react-doctor-before-full.log), [current](evidence/react-doctor-after-full.log).
- No score regression. Existing accessibility, modal, eager chart, provider sequencing and context diagnostics remain advisory; changing those would alter scope. Existing `usePermissions` cleanup error is unchanged; code reads show cleanup delegates to stopPolling and the unmount loop, so this analyzer finding is not a newly introduced leak.
- Newly detected onboarding prop-state warning is the unchanged App first-run effect moved into useOnboarding (high confidence: behavior preserved). Newly exported terminal helper is a low-impact development Fast Refresh advisory; its call/action logic is unchanged and browser assertions cover error actions. Splitting i18n reduced aggregate non-component export diagnostics. No suppressions added and no unrelated accessibility or lifecycle changes made.

## Remaining

Frontend implementation and automated/browser checks are ready for coordinator review and controlled commit. Native/manual scenarios remain explicitly unverified by this frontend worker. No source file exceeds the line target. STATUS and parent change updates belong to the coordinator.

## Coordinator integration

两阶段只读集成审查通过，原生 macOS debug build 通过，STATUS 已记录真实边界。前端源码以 5813965 独立提交；原生手动矩阵在 organize-dictation-core/evidence.md 保持开放。证据日志仅移除行尾空白以通过 git diff --check，诊断/退出结果未改。
