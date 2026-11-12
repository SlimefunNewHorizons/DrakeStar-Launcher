# Arquitectura

DrakeStar es un launcher de Minecraft: **Tauri 2 (Rust) + Svelte 5**. Toda la lógica de juego vive en Rust y no depende de Tauri; la interfaz solo refleja el estado.

```
ui/  (Svelte 5 + TypeScript)
  App · views (Jugar, Cuentas, Instancias, Ajustes, Registros) · components · lib/store.svelte.ts · lib/api.ts
     │  invoke(comando) / eventos: progress, game-state, game-line, msa, report
     ▼
src-tauri/  (glue: comandos, eventos, actualizador, abrir navegador/carpetas)
     ▼
crates/drakestar-mc  (motor, sin Tauri, 24 pruebas)
  manifest · version · install · assets · java · fabric · mods · launch · play
  msa · accounts · offline · instances · settings · support
     ▼
crates/drakestar-net  (descargas https con SHA-256 en streaming)
```

## Flujo de JUGAR (`play::prepare` + `launch::spawn`)

`cuenta → sesión` (refresca el token de Microsoft si hace falta) → `install` (JSON de versión + librerías + natives + assets, todo verificado) → `java::ensure` (JRE gestionado) → mods (Fabric API, rendimiento, Fused) → `options.txt` inicial → `launch::plan` (argumentos JVM/juego, quickPlay) → `spawn` (salida en vivo, token tachado, `launcher-session.log`). Cerrar el juego desde el launcher no cuenta como fallo (`AppState.closing`).

## Módulos del motor

| Módulo | Rol |
|---|---|
| `manifest`, `version` | manifiesto de Mojang (1.20 a 26.2), reglas de librerías/argumentos, `inheritsFrom` de Fabric |
| `server` | servidor local gestionado (DrakesPurpur 26.x / Purpur + Slimefun): descarga, `server.properties`, plan de arranque, espera de puerto |
| `install`, `assets`, `java`, `fabric`, `mods` | descargas verificadas (SHA-1/256/512), JRE Temurin por versión, perfil Fabric, Modrinth, Fused |
| `launch`, `play` | plan de argumentos y proceso del juego, preparación completa |
| `msa`, `accounts`, `offline` | login de Microsoft por código de dispositivo, varias cuentas, tokens aparte (`secrets.json`), UUID sin premium |
| `instances`, `settings` | instancias y plantillas, ajustes (idioma, privacidad, client ID) |
| `support` | registros con rotación, tachador de datos personales, paquete de reporte y envío |

## Crates heredados

`drakestar-core`, `drakestar-app`, `drakestar-cli`, `drakestar-rust-client`, `drakestar-native-example` son del diseño anterior (perfiles, servidor local DrakesPurpur, cliente Rust de Horizon). **La app ya no los usa**; siguen en el workspace y en CI hasta decidir su destino (el cliente Rust volverá cuando exista la integración con Horizon, ver [LAUNCHER.md](LAUNCHER.md#horizon-cliente-rust-minecraft-263)).

## Seguridad

- Todo archivo descargado se verifica con su hash; los mods de Modrinth con SHA-512; Java con SHA-256.
- El actualizador solo instala versiones firmadas con la clave minisign del proyecto.
- Tokens de Microsoft solo en `accounts/secrets.json` (permisos 0600 en Unix) y tachados de cualquier salida.
- Reportes: https obligatorio, tope de 6 MB y datos personales tachados antes de salir del equipo; se pueden desactivar.
- CSP de la ventana: solo recursos propios y los avatares de `mc-heads.net` / `minotar.net`.
