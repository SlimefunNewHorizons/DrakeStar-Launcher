# Pruebas de punta a punta (sin pantalla)

Guiones para validar la app Tauri real con Xvfb + xdotool (probado en WSL2 / Ubuntu 24.04).

Requisitos: `xvfb xdotool imagemagick`, la app compilada (`ui/node_modules/.bin/tauri build --debug --no-bundle`) y una carpeta de datos ya descargada para no esperar a Java y los assets.

```bash
# 1) arrancar Xvfb :99 + la app (deja la sesion abierta; usar otra terminal para el resto)
DS_APP=target/debug/drakestar-launcher DS_E2E_DATA=$HOME/.drakestar-test bash tools/e2e/start.sh
# 2) acciones: click X Y | type TEXTO | key TECLA | wait SEG   (separadas por ;)  y captura opcional
bash tools/e2e/xdo.sh "click 640 547;wait 1" bienvenida
```

Notas: las capturas salen en `DS_SHOTS` (por defecto `/tmp`). Minecraft captura el puntero de X11 dentro de un mundo (no hay gestor de ventanas): pulsa `Escape` antes de hacer clic en el launcher.
