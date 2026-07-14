#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="Codex Island"
PROCESS_NAME="codex-island"
BUNDLE_ID="com.s840207702.codex-island"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/universal-apple-darwin/release/bundle/macos/$APP_NAME.app"
INSTALLED_APP="/Applications/$APP_NAME.app"
APP_BINARY="$INSTALLED_APP/Contents/MacOS/$PROCESS_NAME"

pkill -x "$PROCESS_NAME" >/dev/null 2>&1 || true

cd "$ROOT_DIR"
pnpm install --frozen-lockfile
pnpm tauri:mac:universal

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "未找到 macOS 应用：$APP_BUNDLE" >&2
  exit 1
fi

/usr/bin/ditto "$APP_BUNDLE" "$INSTALLED_APP"

open_app() {
  /usr/bin/open "$INSTALLED_APP"
}

case "$MODE" in
  run)
    open_app
    ;;
  --debug|debug)
    lldb -- "$APP_BINARY"
    ;;
  --logs|logs)
    open_app
    /usr/bin/log stream --info --style compact --predicate "process == \"$PROCESS_NAME\""
    ;;
  --telemetry|telemetry)
    open_app
    /usr/bin/log stream --info --style compact --predicate "subsystem == \"$BUNDLE_ID\""
    ;;
  --verify|verify)
    open_app
    sleep 2
    process_count="$(pgrep -x "$PROCESS_NAME" 2>/dev/null | wc -l | tr -d ' ' || true)"
    if [[ "$process_count" -ne 1 ]]; then
      echo "运行实例数量异常：$process_count" >&2
      exit 1
    fi
    ;;
  *)
    echo "用法：$0 [run|--debug|--logs|--telemetry|--verify]" >&2
    exit 2
    ;;
esac
