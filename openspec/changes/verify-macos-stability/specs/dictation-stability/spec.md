## ADDED Requirements

### Requirement: Confirmed empty selections MUST remain dictation

macOS selection detection MUST distinguish confirmed empty selection from unsupported selection APIs. It MUST NOT simulate copying after confirming an empty selection, because editors can copy a whole line without selecting it.

#### Scenario: Editor copies its current line with no selection

- **WHEN** the focused editor reports an empty selected-text attribute
- **THEN** pressing the primary dictation shortcut stays in polish mode and MUST NOT copy the current line as rewrite context

#### Scenario: Actual selected text exists

- **WHEN** the focused control reports selected text
- **THEN** Audie uses that selected text for rewrite without probing the clipboard

#### Scenario: Native selection is unavailable

- **WHEN** a non-Electron application does not support native selected-text querying
- **THEN** the existing clipboard compatibility probe remains available

#### Scenario: Electron accessibility tree is still starting

- **WHEN** Electron accepts accessibility activation but selected text is still unavailable
- **THEN** this invocation conservatively stays in ordinary dictation without a clipboard probe

### Requirement: Stability evidence MUST identify its actual coverage

Each native scenario MUST compare a controlled target with its own transcript or enhanced output and identify failures or missing evidence separately.

#### Scenario: Manual checks are deferred

- **WHEN** the maintainer cannot perform sleep/wake or WeChat checks
- **THEN** those checks remain pending and MUST NOT be inferred from synthetic input, restart, or unit tests
