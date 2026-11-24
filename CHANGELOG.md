# Cambios

## Sin publicar

**Nuevo**
- **Catálogo Modrinth** por instancia para mods Fabric, resource packs y shaders: filtro de versión, instalar/quitar, dependencias requeridas, Iris para shaders, SHA-512 y protección de archivos externos.
- **Cuentas Ely.by** con 2FA, renovación de token y authlib-injector oficial fijado y verificado; disponibles junto a Microsoft y sin premium. Pendiente validación con cuenta real.
- **Diagnóstico** reúne fallo/etapa/instancia y salida de juego con regreso a Jugar, cuentas o edición. Carpeta de instancia accesible también desde Jugar y Catálogo, con errores visibles.
- Alcance, pruebas y evaluación separada de Starling Android: [docs/STARLING.md](docs/STARLING.md); atribuciones en [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

**Arreglado**
- El launcher conserva `instances.json` si está dañado o no se puede leer. Guardar, retirar o abrir una instancia devuelve el error sin reemplazar sus perfiles por plantillas.
- Si no puede leer las instancias, muestra un aviso persistente en Jugar e Instancias con reintento y acceso a la carpeta de datos. No permite jugar con perfiles obsoletos ni confunde el fallo con un catálogo vacío.

## 0.7.2 — Motor nativo (Horizon / DrakesRust) en Windows, release por CI y purge de datos personales (2026-11-24)

**Arreglado**
- **El motor nativo funciona en Windows** (`horizon.rs`): el modo DrakesRust / Horizon 26.3 ya no fallaba al instalar en Windows ("sin release fijado"). Se fija el paquete por plataforma (`horizon-windows-x64.zip`, `horizon-windows-arm64.zip`, además del `.tar.gz` de Linux y el `.zip` de macOS arm64) y la extracción ya **no depende del `tar` del sistema**: se hace en proceso con las librerías `tar`/`flate2`/`zip` (el paquete de Windows es un `.zip`), quitando la carpeta de la plataforma y rechazando rutas que se salgan del destino. La instalación extrae a un lado y sustituye solo si quedó completa, y marca ejecutables los binarios donde el `.zip` no guarda permisos.
- **Título de ventana**: `wmctrl` es solo X11; el renombrado ahora es un no-op documentado en Windows/macOS (antes se reintentaba 10 s sin sentido) y la variable `WINIT_UNIX_BACKEND` solo se envía en Linux.
- **`--game-dir` en el plan del motor**: la carpeta de juego (mods, saves, opciones) vive en la instancia, como en el modo Java.
- **Credenciales personales fuera del repo**: la vista de muestra del navegador ya no muestra la cuenta de JackStar6677 (mock con nombre y UUID ficticios) y los fixtures de pruebas usan nombres neutros.

**Nuevo**
- **Publicar por CI sin compilar a mano**: `tools/bump-release.mjs` (`npm run bump-release -- X.Y.Z`) sincroniza `Cargo.toml`, `src-tauri/tauri.conf.json`, `ui/package.json`, commitea, etiqueta y empuja; el workflow `release.yml` compila en Windows/Ubuntu/macOS, firma y sube `latest.json`. Guía en [docs/PUBLICAR.md](docs/PUBLICAR.md#publicar-una-versión-con-el-ci-no-compiles-a-mano).

**Pruebas**
- `drakestar-mc`: 41 en verde en Windows (eran 38). Dos pruebas que comparaban rutas con `/` fallaban en Windows por el separador; las nuevas cubren la URL por plataforma y la extracción de `.zip` y `.tar.gz` sin la carpeta raíz.
- **Todo el workspace en verde en Windows** (183 pruebas): tres pruebas de los crates heredados daban por hecho Linux. `drakestar-core`: `-XX:HeapDumpPath` lo manda Mojang solo a Windows. `drakestar-app`: la librería solo-Windows de Fabric entra solo en Windows, y la resolución del manifiesto comprometido se comprueba en las dos caras. `drakestar-rust-client`: los servidores mínimos de dos pruebas cerraban sin leer el saludo del cliente, y en Windows eso provoca un RST que aborta la lectura antes de llegar el paquete.

## 0.7.1 — Zero-Config, Slimefun Local Auto-Connect & Horizon JAVA_HOME fix (2026-11-19)

**Arreglado**
- **Horizon (`JAVA_HOME`)** (`5ffd2c6`): se pasa la raíz del JDK (`JAVA_HOME`) en lugar del binario `bin/java` a `horizon-prepare`, evitando fallos al preparar el motor DrakesRust / Horizon 26.3.
- **Limpieza del servidor local**: si falla el lanzamiento (`spawn`) del cliente Java tras arrancar el servidor local, el proceso de DrakesPurpur se termina de inmediato (`sv.kill()`) sin dejar procesos huérfanos ni puertos ocupados.

**Mejorado**
- **Slimefun en solitario / LAN (QuickPlay End-to-End)**: flujo verificado de extremo a extremo — descarga/arranque del monolito DrakesPurpur (Java 25 gestionado en `127.0.0.1:<puerto>`), espera de `wait_ready`, auto-conexión del cliente vía `--quickPlayMultiplayer 127.0.0.1:<puerto>` y apagado limpio con 0 procesos huérfanos. Se añade el harness de verificación `crates/drakestar-mc/examples/monolith_play_test.rs`.
- **Blindaje Zero-Configuration**:
  - Descarga automática de Eclipse Temurin 17/21/25 gestionada sin requerir Java global.
  - Generación automática de `eula=true` y `online-mode=false` para servidores locales.
  - Límites de RAM por defecto amigables para equipos de escritorio: **3 GB cliente + 3 GB servidor local** en `slimefun-solo` (antes 8 GB cliente + 4 GB servidor) y 3 GB por defecto en nuevas instancias.
- **CI/CD**: `Cargo.lock` sincronizado a `0.7.1` para compilación reproducible con `--locked`.

## 0.7.0 — Feature-Complete / Feature-Freeze Producción (2026-11-18)

Estado oficial: **Feature-Complete (Producción & Estabilidad)**. A partir de esta versión el launcher entra en modo mantenimiento y estabilidad.

**Nuevo**
- **Modo Dual (Java + DrakesRust / Horizon 26.3)**:
  - **Motor Java optimizado**: flags de Aikar (G1GC tuneado) inyectados automáticamente en el lanzamiento del cliente Java (PR #13).
  - **Motor alterno DrakesRust (Horizon 26.3)**: integración nativa (`horizon_status`, `horizon_install`, `horizon_play`, `horizon_stop`) para lanzar el cliente escrito en Rust desde la misma interfaz (PR #12).
- **Monolito Local («Slimefun en solitario / LAN»)**: descarga e inicializa el **monolito DrakesPurpur 26.2** en segundo plano (~22 s en `127.0.0.1` o LAN, Java 25, 9 suites incluidas). Al cerrar el juego o la ventana del launcher (`WindowEvent::Destroyed`), el proceso del servidor local se termina limpiamente (`sv.kill()`) sin procesos zombis ni fugas de puertos.
- **Cliente Rust**: lectura de componentes de chat y visualización clara del motivo de kick (PR #11).
- **CI/CD & Seguridad**: `Cargo.lock` sincronizado a `0.7.0` para compilación reproducible con `--locked` (PR #15) y firma automática minisign para Windows (`.exe`/`.msi`), Linux (`.AppImage`/`.deb`/`.rpm`) y macOS universal (`.dmg`).
- **Validación**: 38/38 tests unitarios de `drakestar-mc` pasando en verde con `--locked`.

## 0.6.0 — launcher rehecho (2026-11-12)

Foco: que se pueda **jugar**. Es un launcher de Minecraft, no de servidores.

**Nuevo**
- Botón **JUGAR** que prepara y abre Minecraft de 1.20 a 26.2: Java gestionado por versión (Temurin 17/21/25), librerías y assets verificados, Fabric con Fabric API, mods de rendimiento y DrakeStar Fused (alfa).
- **Cuentas**: login de Microsoft por código de dispositivo (abre el navegador solo; el código se muestra y se copia con un clic) y cuentas sin premium; varias de cada tipo por PC; los tokens se guardan aparte.
- **Interfaz nueva y simple**: Jugar, Cuentas, Instancias, Ajustes y Registros; bienvenida con aviso de privacidad; español e inglés.
- **Registros y reportes**: logs del launcher (14 días) y del juego; si el juego se cierra con error se crea y envía un reporte solo para depurar, con rutas, correos, IPs y claves tachados (desactivable). Recolector en `web.drakescraft.cl/api/launcher/report`.
- **Actualización automática firmada** (minisign) desde las releases de GitHub.
- Modo **sin conexión** con lo ya instalado; primer arranque sin la pantalla del narrador.
- Instaladores: Windows `.exe` y `.msi`, Linux `.deb`/`.rpm`/`.AppImage`, macOS `.dmg`.

**Arreglado**
- «Slimefun solitario/LAN» solo levantaba un servidor y no abría Minecraft; ajustes sin destino; GUI que abrumaba; login sin gestión de cuentas.
- Cerrar el juego desde el launcher ya no cuenta como fallo.

**Cambia**
- Las 0.5.x no tienen actualizador: instalar esta una vez a mano. El diseño anterior (perfiles, servidor local, cliente Rust) ya no está en la interfaz; sus crates siguen en el repo hasta decidir su destino.

**Pendiente**: client ID propio de Azure aprobado por Mojang (hoy usa uno temporal), host Bukkit real en Fused, integración con Horizon. Ver [docs/LAUNCHER.md](docs/LAUNCHER.md) y [docs/ROADMAP.md](docs/ROADMAP.md).

## 0.5.2 y anteriores

Ver [los releases](https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases).
