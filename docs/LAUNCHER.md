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

## DrakeStar Fused (1.20 a 26.2)

Repo `drakestar-fused`. Desde la **1.0.0-alpha.3** es un solo `.jar` de Fabric **sin mixins** (usa los eventos `SERVER_STARTING/STOPPING` de Fabric API, que no dependen de los nombres internos de Minecraft), así que sirve igual en versiones ofuscadas (1.20 a 1.21.11) y no ofuscadas (26.x). Probado con el launcher en **1.20.1, 1.21.1, 1.21.11, 26.1 y 26.2** (carga, menú y creación de mundo en 1.21.1). Las alpha.1 y alpha.2 usaban mixins sin remapear y **tumban el juego al arrancar**: el launcher las ignora y borra sus `.jar`. Estado real: prepara el entorno del servidor integrado (carpeta `plugins/`, puente Rust opcional) pero **el host Bukkit todavía no ejecuta plugins**; la interfaz lo marca como alfa. Publicar: subir `mod_version` en `gradle.properties` y empujar la etiqueta `vX.Y.Z` (el flujo compila, comprueba el jar con `release-artifacts/check_jar.py` y crea el release con `drakestar-fused-fabric-universal-<versión>.jar`). Hasta que exista un release válido, el launcher abre la instancia sin Fused y lo avisa.

## Horizon (cliente Rust, Minecraft 26.3)

No integrado. Su API de lanzamiento para launchers de terceros exige una app de Azure propia aprobada por Mojang (ver arriba), los archivos de 26.3 solo para propietarios y Java 25; los servidores anteriores solo con `HORIZON_OLDER_SERVERS=1` (experimental). Cuando exista el client ID propio, se añade como motor de una instancia (`horizon-prepare` + `horizon-client --session-stdin`).

## Pruebas de punta a punta

`tools/e2e/` trae los guiones usados para validar la app real sin pantalla (Xvfb + xdotool, p. ej. en WSL): arrancan la app Tauri compilada, hacen clic y capturan. Cuidado: Minecraft captura el puntero de X11 dentro de un mundo; pulsa `Escape` antes de hacer clic en el launcher. Pruebas de motor: `cargo test -p drakestar-mc` y `cargo run -p drakestar-mc --example play_test -- <mc> [vanilla|fabric] [segundos] [carpeta]`.

## Problemas frecuentes

- **No abre la ventana en Windows**: falta WebView2 (el instalador lo descarga; sin internet, instalar «WebView2 Runtime» de Microsoft).
- **«Microsoft rechazó el client ID» / 403 Invalid app registration**: falta la app propia aprobada por Mojang (sección Azure).
- **El juego se cierra al abrir**: *Registros → Juego*; el aviso ofrece desactivar Fused o enviar el reporte.
- **Antivirus bloquea Java o el launcher**: añadir excepción a la carpeta de datos.
- **Sin internet**: lo ya descargado funciona sin conexión (se omiten mods y comprobaciones y se avisa).
