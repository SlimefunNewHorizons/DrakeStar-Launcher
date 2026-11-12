#!/bin/bash
# Arranca Xvfb :99 y la app Tauri compilada; se queda vivo para que otras llamadas usen DISPLAY=:99.
pkill -f "Xvfb :99" 2>/dev/null || true
APP="${DS_APP:-target/debug/drakestar-launcher}"
pkill -f "$APP" 2>/dev/null || true
sleep 1
DATA="${DS_E2E_DATA:-$HOME/.drakestar-test}"
rm -f "$DATA/settings.json"   # primer arranque: vuelve a salir la bienvenida
Xvfb :99 -screen 0 1280x800x24 +extension GLX +render -noreset >/dev/null 2>&1 &
sleep 2
unset WAYLAND_DISPLAY
export DISPLAY=:99 GDK_BACKEND=x11 DRAKESTAR_DATA_DIR="$DATA" WEBKIT_DISABLE_DMABUF_RENDERER=1 LIBGL_ALWAYS_SOFTWARE=1
[ -n "$DS_MSFT_CLIENT_ID" ] && export DRAKESTAR_MSFT_CLIENT_ID="$DS_MSFT_CLIENT_ID"
"$APP" > "${DS_LOG:-/tmp/drakestar-e2e.log}" 2>&1 &
echo "app pid $!"
sleep "${DS_E2E_SECS:-1500}"
