#!/usr/bin/env bash
# Dev-only: codesign the cargo-built binary with a STABLE Apple Development
# identity, then exec it. Invoked by cargo as the `runner` (see .cargo/config.toml)
# for `tauri dev` / `cargo run` on apple-darwin.
#
# Why: a stable signature => stable macOS "designated requirement" (pinned to the
# bundle id via -i + the signing cert's leaf) => the Keychain "Always Allow" ACL
# keeps matching across rebuilds with the same cert, so the prompt stops recurring.
# (It re-prompts once only if the signing cert itself is renewed, ~yearly.)
#
# Contributors without the cert: set nothing and the binary runs UNSIGNED (one-line
# warning), exactly as before — nobody is blocked.
#
# Identity resolution: $AUDIE_SIGN_IDENTITY, else <repo-root>/.cargo/sign-identity
# (gitignored, single line). Value = what `security find-identity -v -p codesigning`
# shows in quotes, or its 40-char SHA-1 hash (most robust with multiple certs).
set -euo pipefail

BIN="$1"        # cargo passes the built binary path as the first arg
shift || true   # remaining args are forwarded to the app

# Only sign on macOS; elsewhere just run it.
if [[ "$(uname -s)" != "Darwin" ]]; then
  exec "$BIN" "$@"
fi

# Resolve this script's dir -> repo root (script lives in <root>/scripts/).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

IDENTITY="${AUDIE_SIGN_IDENTITY:-}"
if [[ -z "$IDENTITY" && -f "$REPO_ROOT/.cargo/sign-identity" ]]; then
  IDENTITY="$(grep -v '^[[:space:]]*#' "$REPO_ROOT/.cargo/sign-identity" | sed '/^[[:space:]]*$/d' | head -n1 || true)"
fi

if [[ -z "$IDENTITY" ]]; then
  echo "[dev-sign-run] AUDIE_SIGN_IDENTITY not set and .cargo/sign-identity absent; running UNSIGNED (keychain may re-prompt)." >&2
  exec "$BIN" "$@"
fi

# --force: replace the existing ad-hoc/linker signature.
# -i com.aubergine.audie: pin the identifier half of the designated requirement to
#   the stable bundle id (the default reuses the per-build audie-<hash>, so the DR
#   would drift every rebuild). With a fixed identifier + the same signing cert,
#   the DR stays constant across rebuilds, so the keychain ACL keeps matching.
if ! codesign --force -i com.aubergine.audie --sign "$IDENTITY" "$BIN" >&2; then
  echo "[dev-sign-run] codesign failed for identity '$IDENTITY'; running UNSIGNED." >&2
  exec "$BIN" "$@"
fi

exec "$BIN" "$@"
