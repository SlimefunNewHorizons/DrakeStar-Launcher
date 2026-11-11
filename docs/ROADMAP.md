# Roadmap

**v0.1 (hecho):** núcleo con tests, CLI, esquemas, ejemplos, marca vectorial, CI, documentación.

**v0.2 — Local Forge usable (hecho en backend, falta prueba end-to-end contra JAR completo de DrakesPurpur):**
- hecho y probado: `drakestar-net` (descarga https + SHA-256, probada contra un servidor HTTP local), gestor de procesos (arrancar/parar con `stop` y escalada a kill, stdout/stderr en streaming, stdin, logs en disco; tests para Unix `sh` y Windows `cmd /C`, además de JAR de juguete en Java 25 real), EULA, `server.properties`, listado y activar/desactivar plugins, instalación de preset Slimefun (`install_slimefun_preset`), listado de mundos y copias de seguridad (`backup_world`).
- pendiente: prueba automática en CI contra un **JAR completo de DrakesPurpur**; caché de descargas compartida y rollback; manifiesto oficial publicado.

**v0.3 — Shell gráfico (hecho en alcance alfa):** Tauri 2 + Svelte con todas las rutas de `ui/routes.json`, tema DrakeStar, perfiles CRUD + import/export, instancias, consola en vivo, `preserveSymlinks` en Vite para compilación multiplataforma en Windows/junctions y Linux. Verificado en Linux (ventana real bajo Xvfb). Pendiente: bundles/instaladores por plataforma, selector nativo de archivos para import/export (hoy: pegar/elegir archivo en la UI y carpeta `exports/`), pruebas automáticas de UI contra el backend real (hoy contra el mock), revisión de accesibilidad con lector de pantalla.

**v0.4 — Sistema de mods y cliente nativo (completado):** el launcher decide qué mods entran, valida integridad SHA-256 de librerías, y carga código nativo además de Java.
- **hecho y probado**: instalación y lanzamiento del cliente Java con **Fabric** (Mojang + Fabric verificados por hash; ejecutado con mods reales: Sodium, Iris Shaders, Distant Horizons, fabric-api). **Sistema de mods propio**: manifiesto `fabric.mod.json` sin modificar, dialecto semver de Fabric, resolución de dependencias, **cargador** que descubre mods (incluidos sus jar-in-jar), verifica SHA-256 contra `mods.lock.json` y deshabilita lo que no carga. **Intermediario** con ABI versionado y catálogo de hooks. **Adaptador nativo** (`cdylib`) y **motor de hooks** que ejecuta mods Rust de punta a punta. **Cliente Rust nativo** (`drakestar-rust-client`) que el launcher instala por canal, verifica y lanza, probado contra mini-servidor de login, zlib compression, keepalive persistente y decodificación de interfaces Slimefun. **Análisis pre-arranque**: dependencias incumplidas, conflictos, IDs duplicados y colisiones de clases objetivo de Mixin (lo que hace crashear Fabric en caliente).

**v0.5 & v0.5.1 — Online, Pipeline Visual & Autenticación Soberana (completado):**
- **Autenticación Microsoft Online-Mode (OAuth 2.0 Device Code)**: Cadena completa de canje de tokens (Device Code → Xbox Live RPS → XSTS → Minecraft Services Profile). Cálculo de `server_id_hash` con precisión estricta en complemento a dos (BigInteger Java). Interfaz gráfica interactiva en Tauri (`MicrosoftAuthModal.svelte`) con código temporal, copiado al portapapeles, polling de estado en tiempo real y desvinculación segura.
- **Pipeline Visual de Renderizado (OpenGL / Vulkan / Shaders / dGPU)**:
  - Soporte de renderizado para `Opengl` (Sodium, Iris Shaders, vanilla LWJGL) y `Vulkan` (VulkanMod nativo).
  - Configuración de resolución de ventana (`width`, `height`), modo pantalla completa (`fullscreen`) y flags de offload en GPU dedicada para Linux (`DRI_PRIME=1`, `__NV_PRIME_RENDER_OFFLOAD=1`).
  - Auto-conexión instantánea a servidores (`--server <host> --port <port>`).
- **Telemetría y Estado Online**: Ping TCP nativo y protocolo Minecraft Server List Ping (`mc_status_ping`, SLP `0x00`) con latencia, conteo de jugadores en vivo y MOTD sin códigos de formato `§`.
- **Importador de Instancias Prism**: Detección y migración automática de instancias locales de Prism Launcher con transferencia de configuraciones, mods y hashes.

**v0.5.2 — Puente Fused Multi-Loader, Selector de Presets Personal y Respaldo de Mundos (completado):**
- **Puente Multi-Loader `drakestar-fused` (Fabric / NeoForge / Forge 1.21.1)**:
  - Integración del mod híbrido en el pipeline del launcher con soporte de ciclo de vida completo (`FusedLifecycleListener`), compatibilidad binaria Bukkit/Spigot/Paper en singleplayer (`FusedCraftServer`, `FusedPluginLoader`, `FusedCrashHandler`), y resolución nativa dinámica de `Slimefun-Rust` FFI (`libdrakes_hybrid_ffi`).
  - Pipeline de compilación y empaquetado multi-loader sin Gradle pesado en release (`build_all_loaders.sh`), generando artefactos listos para producción para Fabric, NeoForge y Forge.
- **Selector de Presets Personal (Requerimiento Chagui)**:
  - Selector interactivo en UI (`ProfileEditor.svelte`) para elegir entre mundo local con `Slimefun Classic` o `Vanilla Puro (Sin plugins)`, permitiendo a desarrolladores y jugadores correr entornos locales de pruebas ultralivianos o mundos técnicos vanilla.
- **Respaldo Instantáneo de Mundos**:
  - Acción interactiva en `ui/src/views/Worlds.svelte` conectada vía Tauri IPC a `backupWorld`, permitiendo crear copias de seguridad de mundos Purpur 26.2 antes de cambios de versiones o adición de plugins.
- **Modernización y Pulido de UI**:
  - Erradicación de notas de versión pendientes obsoletas en vista Java (`🎮 Lanzar Minecraft` directo), modal de reporte de errores integrado con telemetría v0.5.2 y navegación optimizada.

**v0.6 — Actualizaciones & Releases:** manifiestos firmados, canales por componente, rollback. *Hoy: resolución por canal desde un manifiesto importado y catálogo Modrinth integrado.*

**v1.0 — DrakesRust:** conexión nativa a DrakesPurpur 26.2 demostrada con KeepAlive automático, compresión de paquetes Zlib y apertura de GUIs de Slimefun.

**Análisis pre-arranque:** el cargador detecta dependencias incumplidas (con el dialecto semver de Fabric), conflictos declarados, IDs duplicados y **colisiones de clases objetivo de Mixin** — que es lo que hace crashear Fabric en caliente. Se suma al `modpack` delta-sync: uno decide qué se descarga, el otro qué se carga.

Pendiente transversal: instaladores por plataforma, pruebas con jugadores reales en producción, Geyser/Bedrock (fuera de alcance), internacionalización completa (i18n).
