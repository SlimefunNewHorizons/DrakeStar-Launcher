# Guía de operación de DrakeStar

Todo lo que hace falta para operar, publicar y depurar el launcher. Qué es y cómo se usa: [README](../README_ES.md).

## Qué hace

Un launcher de Minecraft (no de servidores): **JUGAR** prepara y abre el juego.

1. Cuenta (Microsoft con refresco automático, o sin premium) → sesión de juego.
2. Java: Eclipse Temurin JRE de Adoptium, **uno por versión mayor** (17 para 1.20.x, 21 para 1.21.x, 25 para 26.x) en `runtimes/java-<n>`, verificado con SHA-256. Nunca usa el Java del sistema.
3. Minecraft: `version_manifest_v2` de Mojang, librerías con reglas por sistema, *natives*, assets; todo con SHA-1 y descargas en paralelo. Rango soportado: **1.20 a 26.2** (solo releases).
4. Fabric (opcional): loader + Fabric API (Modrinth) + mods de rendimiento (Modrinth, SHA-512) + DrakeStar Fused (releases de GitHub).
5. Lanza el juego con `--quickPlayMultiplayer` / `--quickPlaySingleplayer` según la instancia y recoge su salida en vivo.

Carpeta de datos (`DRAKESTAR_DATA_DIR` para cambiarla): `versions/ libraries/ assets/ runtimes/ instances/<id>/ logs/ support/ accounts/ settings.json instances.json`.

## Cuentas y Azure (IMPORTANTE: pendiente)

El login de Microsoft usa el flujo de **código de dispositivo** (`consumers`, scope `XboxLive.signin offline_access`): el launcher abre el navegador en `microsoft.com/link` con el código ya puesto (`?otc=`; si la página no lo rellena, el código se muestra en una ventana con botón *Copiar*), sondea hasta que inicias sesión y guarda el *refresh token* aparte de la lista de cuentas. Después encadena XBL → XSTS → servicios de Minecraft.

**Hoy usa el client ID de otro launcher (Prism) de forma temporal** (`LEGACY_CLIENT_ID` en `crates/drakestar-mc/src/settings.rs`). La interfaz lo avisa en *Cuentas*. Para dejarlo definitivo:

1. En <https://entra.microsoft.com> → *App registrations* → *New registration*: nombre «DrakeStar», **solo cuentas personales de Microsoft**; en *Authentication* activar **Allow public client flows**; no hace falta redirect URI.
2. Pedir a Mojang que apruebe el `Application (client) ID`: <https://aka.ms/mce-reviewappid> (sin esto, `login_with_xbox` responde 403 «Invalid app registration» y el launcher lo explica).
3. Guardar el ID como **variable del repositorio** `DRAKESTAR_MSFT_CLIENT_ID` (Settings → Secrets and variables → Actions → Variables). El flujo de release lo compila dentro de la app (`option_env!`); sin variable cae al heredado. Para pruebas: variable de entorno `DRAKESTAR_MSFT_CLIENT_ID` o *Ajustes → Avanzado*.

## Registros y reportes de error

- `logs/launcher-AAAA-MM-DD.log` (UTC, 14 días, también los *panics*); el juego en `instances/<id>/logs/latest.log` y `launcher-session.log` (salida cruda con el token tachado).
- Al cerrarse el juego con error (no si lo cierras tú desde el launcher) se crea `support/report-<n>.zip` y, si *Enviar reportes* está activo (por defecto sí, se avisa en la primera apertura), se envía a `https://web.drakescraft.cl/api/launcher/report` (`POST`, `Content-Type: application/zip`, máx. 6 MB, https obligatorio).
- El `.zip` lleva `context.json` (versión de la app y de Minecraft, SO, nombres de mods, código de salida), los últimos registros del launcher, `latest.log`, `launcher-session.log` y hasta 2 *crash reports*, **todo pasado por el tachador** (`support::redact`: tokens, JWT, rutas de usuario, correos, IPs).
- Recolector (repo `drakescraft-web`, `lib/launcher-reports.js` + ruta en `server.js`): valida firma ZIP, 6 reportes por hora y por IP, guarda en `<DATA_DIR>/launcher-reports/<fecha>_<id-equipo>_v<versión>.zip` (30 días, máx. 2000), **no guarda la IP**. En Star: `/opt/stacks/drakescraft-web/data/launcher-reports/`.
  - Ver los últimos: `ssh star 'ls -t /opt/stacks/drakescraft-web/data/launcher-reports | head'` y `unzip -p <zip> context.json game/latest.log | less`.
  - Desplegar cambios del recolector: `ssh star ~/ai-hub/scripts/desplegar_web.sh` (el repo de trabajo debe estar en `main` y publicado).

## Actualizaciones automáticas

Plugin `tauri-plugin-updater`; endpoint `https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/latest/download/latest.json`; firmas minisign (la clave pública va en `src-tauri/tauri.conf.json`). La clave **privada** vive fuera del repo (`%USERPROFILE%\.drakestar-keys\updater.key`, copia en el secreto `TAURI_SIGNING_PRIVATE_KEY` del repositorio; sin contraseña). Si se pierde, hay que publicar un instalador nuevo con otra clave pública: las apps ya instaladas no podrán actualizarse solas.

**Publicar una versión**

1. Subir la versión en `Cargo.toml` (`[workspace.package]`), `src-tauri/tauri.conf.json` y `ui/package.json` (las tres iguales).
2. `git tag vX.Y.Z && git push origin vX.Y.Z` → el flujo *Release* comprueba que etiqueta y versión coinciden, compila Windows (`.exe` NSIS + `.msi`), Linux (`.deb`, `.rpm`, `.AppImage`) y macOS (`.dmg`, universal), los firma y sube `latest.json`.
3. Las apps instaladas (0.6 o más nuevas) lo ven al abrir. **La 0.5.x no tiene actualizador: se reinstala una vez a mano.**

El `.msi` instala por equipo (pide administrador al actualizar); el `.exe` (NSIS, modo `both`) puede instalarse por usuario y no pide permisos: es el recomendado. El actualizador respeta el tipo con el que se instaló. WebView2 se descarga solo si falta (`downloadBootstrapper`).

## Servidor local gestionado («Slimefun en solitario / LAN»)

Destino de instancia `local` (`{ "kind": "local", "port": 25570, "lan": false }`). Al pulsar JUGAR el launcher prepara el servidor, lo arranca sin ventana, espera a que abra el puerto y lanza el cliente directo a `127.0.0.1:<puerto>`; al cerrar el juego (o el launcher) mata el servidor.

- **26.x**: baja el último release estable de `SlimefunNewHorizons/DrakesPurpur` (`purpur-paperclip-*.jar`, ~95 MB, tamaño verificado). Es el **monolito**: Purpur + núcleo Drakes + 9 suites con Slimefun y los 94 addons; no se instalan plugins aparte. Java 25 gestionado, `--enable-native-access=ALL-UNNAMED`; el puente Rust viene apagado por defecto en el kernel (con respaldo en Java).
- **Antes de 26**: Purpur (API de PurpurMC, MD5 verificado) + `Slimefun-*-<mc>-SNAPSHOT.jar` de Slimefun4-Drake si existe para esa versión (si no, avisa y abre sin Slimefun).
- Carpeta: `<root>/instances/<id>/server/` (mundo, `plugins/`, `server.properties`). Se impone `server-port`, `server-ip` (127.0.0.1 salvo `lan`), `online-mode=false`; el resto de `server.properties` se respeta. Se escribe `eula=true` (la nota de la instancia avisa de que jugar implica aceptar el EULA de Mojang).
- RAM: el servidor usa la mitad de la memoria de la instancia (mín. 3 GB en monolito, 1,5 GB en Purpur). Registro: `server/logs/launcher-server.log`.
- Plantillas antiguas «Slimefun solitario» con Fused en el menú migran solas a esta.
- No probado aún con un jugador real desde la interfaz; sí el arranque del monolito (`Done` en ~22 s) y de Purpur 1.21.11 + Slimefun.

## Por qué Fused no es el camino para Slimefun

Un Bukkit dentro del servidor integrado no cubre el objetivo (Fabric, Forge y NeoForge, 1.20 a 26.2, monolito y optimizaciones Rust del núcleo): Cardboard es solo Fabric 1.21.x (probado en 1.21.11: carga, pero Slimefun falla al crear cabezas de jugador, `CustomGameProfile.apply`), Arclight llega a 1.21.1, y las optimizaciones viven en el núcleo de DrakesPurpur. Por eso se ejecuta el DrakesPurpur real aparte y automático. Fused queda como mod opcional alfa.

## DrakeStar Fused (1.20 a 26.2)

Repo `drakestar-fused`. Desde la **1.0.0-alpha.3** es un solo `.jar` de Fabric **sin mixins** (usa los eventos `SERVER_STARTING/STOPPING` de Fabric API, que no dependen de los nombres internos de Minecraft), así que sirve igual en versiones ofuscadas (1.20 a 1.21.11) y no ofuscadas (26.x). Probado con el launcher en **1.20.1, 1.21.1, 1.21.11, 26.1 y 26.2** (carga, menú y creación de mundo en 1.21.1). Las alpha.1 y alpha.2 usaban mixins sin remapear y **tumban el juego al arrancar**: el launcher las ignora y borra sus `.jar`. Estado real: prepara el entorno del servidor integrado (carpeta `plugins/`, puente Rust opcional) pero **el host Bukkit todavía no ejecuta plugins**; la interfaz lo marca como alfa. Publicar: subir `mod_version` en `gradle.properties` y empujar la etiqueta `vX.Y.Z` (el flujo compila, comprueba el jar con `release-artifacts/check_jar.py` y crea el release con `drakestar-fused-fabric-universal-<versión>.jar`). **Publicada: [v1.0.0-alpha.3](https://github.com/SlimefunNewHorizons/drakestar-fused/releases/tag/v1.0.0-alpha.3)**; el launcher la instala sola en las instancias con Fused (y borra los jars de las alpha.1/2). Si no hubiera ningún release válido, abre la instancia sin Fused y lo avisa.

## Horizon (cliente Rust, Minecraft 26.3)

No integrado. Su API de lanzamiento para launchers de terceros exige una app de Azure propia aprobada por Mojang (ver arriba), los archivos de 26.3 solo para propietarios y Java 25; los servidores anteriores solo con `HORIZON_OLDER_SERVERS=1` (experimental). Cuando exista el client ID propio, se añade como motor de una instancia (`horizon-prepare` + `horizon-client --session-stdin`).

## Pruebas de punta a punta

`tools/e2e/` trae los guiones usados para validar la app real sin pantalla (Xvfb + xdotool, p. ej. en WSL): arrancan la app Tauri compilada, hacen clic y capturan. Cuidado: Minecraft captura el puntero de X11 dentro de un mundo; pulsa `Escape` antes de hacer clic en el launcher. Pruebas de motor: `cargo test -p drakestar-mc` y `cargo run -p drakestar-mc --example play_test -- <mc> [vanilla|fabric] [segundos] [carpeta]`.

## Verificación de la 0.6.0 (publicada el 2026-11-12)

Probado sobre los archivos **publicados** (no solo sobre compilaciones locales):

| Qué | Cómo | Resultado |
|---|---|---|
| Flujo de release | etiqueta `v0.6.0` → Linux, Windows y macOS en verde; `latest.json` con firma en 11 plataformas | ok |
| Windows (`.msi`) | extraído con `msiexec /a` (sin instalar), WebView2 + `tools/e2e/cdp-shot.mjs`: bienvenida, cuenta sin premium, JUGAR → Minecraft 1.21.11 abierto, «Cerrar el juego» sin falso fallo (código 1 de `TerminateProcess`), «Buscar actualizaciones» → «Ya tienes la última versión» | ok |
| Linux (`.AppImage`) | WSL2 + Xvfb: arranca y muestra la interfaz | ok |
| Actualizador | app compilada con versión 0.5.99 detecta la 0.6.0 en el `latest.json` publicado (banner y botón «Actualizar y reiniciar») | detección ok |
| Web | `POST /api/launcher/report` en producción → 202; el zip se lee desde Star (0644) | ok |
| Fused | jar publicado verificado por SHA-256 y probado en 1.21.1 y 26.2 (antes, 1.20.1, 1.21.11 y 26.1) | ok |

**No probado** (necesita máquinas limpias o cuentas reales): instalación real del `.msi`/`.exe` y del `.deb`/`.rpm`, macOS (`.dmg`, no está firmado ni notarizado), la **instalación** de una actualización (solo su detección) y el login de Microsoft completo (falta el client ID propio).

## Problemas frecuentes

- **Windows SmartScreen avisa «editor desconocido»** al abrir el instalador: no está firmado con un certificado de editor (cuesta dinero). *Más información → Ejecutar de todas formas*. En macOS: clic derecho → *Abrir*.
- **Instalaste con el `.msi` y quieres actualizar**: instala el `.msi` nuevo encima (mismo identificador, se reemplaza). Mezclar `.msi` y `.exe` en el mismo equipo deja dos instalaciones.

- **No abre la ventana en Windows**: falta WebView2 (el instalador lo descarga; sin internet, instalar «WebView2 Runtime» de Microsoft).
- **«Microsoft rechazó el client ID» / 403 Invalid app registration**: falta la app propia aprobada por Mojang (sección Azure).
- **El juego se cierra al abrir**: *Registros → Juego*; el aviso ofrece desactivar Fused o enviar el reporte.
- **Antivirus bloquea Java o el launcher**: añadir excepción a la carpeta de datos.
- **Sin internet**: lo ya descargado funciona sin conexión (se omiten mods y comprobaciones y se avisa).

## Importar instancias y elegir memoria

En **Instancias → Importar instancia**, pega la ruta absoluta de una instancia o un
archivo exportado y pulsa **Revisar**. Se muestran nombre, versión de Minecraft,
loader y RAM antes de **Importar copia**.

- **Prism / MultiMC:** carpeta con `mmc-pack.json` e `instance.cfg`, o ZIP exportado;
  admite el directorio de juego `.minecraft/` y `minecraft/`.
- **CurseForge:** carpeta de una instancia **ya instalada**, con
  `minecraftinstance.json` y sus mods descargados. No admite el ZIP del catálogo
  (`manifest.json`), que requiere resolver descargas de CurseForge.
- **Modrinth:** archivo `.mrpack` exportado, con `modrinth.index.json`;
  descarga archivos de cliente desde `https://cdn.modrinth.com/`, comprueba SHA-1 y
  tamaño, y aplica `overrides/` seguido de `client-overrides/`. Omite archivos
  exclusivos del servidor. Otros hosts de descarga se rechazan explícitamente.

Solo admite **Vanilla y Fabric**, dentro del rango de Minecraft del launcher.
Forge, NeoForge, Quilt y componentes Prism desconocidos se rechazan antes de
crear una instancia. La versión de Fabric importada se conserva al jugar.

Se copian mods, configuraciones, partidas, recursos, shaders, capturas, esquemas,
scripts y opciones/servidores a una carpeta nueva `instances/importada-N`.
La instancia original y las existentes se conservan. No se importan cuentas,
logs, comandos de inicio ni argumentos JVM. Hay un límite de 4 GiB; se rechazan
rutas absolutas, escapes de directorio, enlaces simbólicos y archivos especiales.
Si falla la copia o una descarga, no se registra la instancia y se retiran los
archivos parciales. Las importaciones no corren durante una partida.

Las instancias nuevas e importadas reciben un presupuesto basado en la RAM física
(Windows, macOS y Linux): la mitad del equipo, hasta 6 GiB recomendados. Por
ejemplo, 4/8/16 GiB de RAM recomiendan 2/4/6 GiB de heap. El deslizador permite
ajustarlo hasta la mitad de RAM (máximo 64 GiB); **Usar memoria recomendada**
restaura el presupuesto. Las instancias guardadas conservan su ajuste manual.
Este presupuesto es el heap del cliente: el servidor local también necesita RAM.
