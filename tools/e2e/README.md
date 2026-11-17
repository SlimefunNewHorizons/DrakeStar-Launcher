# Pruebas de punta a punta (sin pantalla)

Guiones para validar la app Tauri real con Xvfb + xdotool (probado en WSL2 / Ubuntu 24.04).

Requisitos: `xvfb xdotool imagemagick`, la app compilada (`ui/node_modules/.bin/tauri build --debug --no-bundle`) y una carpeta de datos ya descargada para no esperar a Java y los assets.

Para revisar el empaquetado Linux sin instalar ni ejecutar la app:

```bash
python3 tools/e2e/check-linux-release.py DrakeStar_0.6.0_amd64.deb --version 0.6.0
```

Requiere `dpkg-deb` y `readelf`. Comprueba version, entrada de escritorio y dependencias GTK/WebKit del ejecutable, e imprime sus SHA-256. Tambien acepta un ELF suelto para detectar el CLI empaquetado por error en 0.5.0. Es una comprobacion estatica: no acredita render, IPC, firma ni actualizacion real en una maquina limpia.

## Instalación y actualización Debian en Ubuntu limpio

`linux-install/` instala los `.deb` oficiales en un contenedor nuevo de Ubuntu
24.04 (imagen base fijada por digest), comprueba versión y entrada de escritorio,
arranca cada versión con un usuario sin privilegios y conserva un archivo
centinela **sintético** dentro de `instances/.../saves` durante la actualización
por `dpkg`. Comprueba que el ejecutable instalado coincide con el del paquete;
el centinela no representa una partida jugable. No monta datos del usuario.

```bash
mkdir -p /tmp/drakestar-packages /tmp/drakestar-install-evidence
gh release download v0.5.2 -R SlimefunNewHorizons/DrakeStar-Launcher --pattern '*.deb' --dir /tmp/drakestar-packages
gh release download v0.6.0 -R SlimefunNewHorizons/DrakeStar-Launcher --pattern '*.deb' --dir /tmp/drakestar-packages
~/ai-hub/scripts/build_seguro.sh 25 "$PWD" docker build -t drakestar-installer-smoke tools/e2e/linux-install
docker run --rm --init --cpus 1 --memory 2g --pids-limit 512 --network none \
  -v /tmp/drakestar-packages:/packages:ro \
  -v /tmp/drakestar-install-evidence:/evidence \
  drakestar-installer-smoke /packages/DrakeStar_0.5.2_amd64.deb /packages/DrakeStar_0.6.0_amd64.deb
```

`result.json`, logs de `dpkg`, logs de sesión y dos capturas quedan en la carpeta
de evidencia. El ensayo exige una ventana visible cuyo PID pertenece al launcher
y que siga vivo después de capturar; **hay que revisar las imágenes** para
confirmar contenido, versión e IPC. Una ventana vacía no acredita render.
La preparación usa red para instalar dependencias; el ensayo exige `--network
none`. Solo en este contenedor desechable, WebKit usa el aislamiento exterior
de Docker en lugar de su sandbox de namespaces y render por software en Xvfb.
La ejecución directa en el host se rechaza antes de instalar.

Esto comprueba el instalador `.deb` y la actualización mediante el gestor de
paquetes. Para 0.6.0, también desactiva los reportes y pulsa «Empezar» mediante
clics X11 en una ventana de 1100×720 (coordenadas propias de esa versión):
exige que el backend guarde ambos ajustes desde sus
valores iniciales y captura `onboarding.png`. No exporta identificadores de
dispositivo ni escribe esos ajustes desde el guion. Quedan aparte el
actualizador firmado de la aplicación, `.rpm`, `.msi`,
Minecraft y la instalación en una máquina física o VM con escritorio completo.
Los artefactos del actualizador y su firma se describen en la
[documentación oficial de Tauri](https://v2.tauri.app/plugin/updater/).

Regresión del propio capturador, sin abrir ventanas ni instalar paquetes:

```bash
python3 -m unittest discover -s tools/e2e/linux-install -p 'test_*.py' -v
```

Comprueba que una salida durante la captura nunca escribe evidencia de éxito y
que tanto el launcher como el gestor de ventanas se limpian en ambos caminos.

```bash
# 1) arrancar Xvfb :99 + la app (deja la sesion abierta; usar otra terminal para el resto)
DS_APP=target/debug/drakestar-launcher DS_E2E_DATA=$HOME/.drakestar-test bash tools/e2e/start.sh
# 2) acciones: click X Y | type TEXTO | key TECLA | wait SEG   (separadas por ;)  y captura opcional
bash tools/e2e/xdo.sh "click 640 547;wait 1" bienvenida
```

Notas: las capturas salen en `DS_SHOTS` (por defecto `/tmp`). Minecraft captura el puntero de X11 dentro de un mundo (no hay gestor de ventanas): pulsa `Escape` antes de hacer clic en el launcher.

## Windows (WebView2)

`cdp-shot.mjs` captura la interfaz de la app de Windows por el protocolo de depuracion de WebView2 (sin tocar el escritorio): ver la cabecera del archivo. Sirve para comprobar un instalador recien publicado sin instalarlo (extraer el `.msi` con `msiexec /a` si el sistema lo permite) y para ejecutar la app con datos aislados (`DRAKESTAR_DATA_DIR`).
