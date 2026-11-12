# Roadmap

## v0.6 — Launcher rehecho (hecho)

Foco: **que se pueda jugar**. Un solo botón JUGAR, cuentas Microsoft y sin premium (varias), Java y mods automáticos, Minecraft 1.20 a 26.2, registros con reporte automático opcional, actualización automática firmada, instaladores para todos los sistemas, interfaz nueva y simple (es/en).

Verificado: pruebas del motor (24), lanzamiento real en Linux (Fabric 1.20.1, 1.21.1, 1.21.11, 26.1, 26.2; vanilla 26.1/26.2), lanzamiento real en Windows (1.21.11, Java 21 gestionado desde `.zip`), la app Tauri completa con interfaz real (bienvenida, cuenta sin premium, JUGAR, mundo nuevo con Fused, cerrar el juego, registros, ajustes).

## Siguiente

1. **Client ID propio de Azure aprobado por Mojang** (hoy temporal, ver [LAUNCHER.md](LAUNCHER.md#cuentas-y-azure-importante-pendiente)). Bloquea que el login de Microsoft sea definitivo.
2. **Publicar** launcher v0.6.0 y Fused v1.0.0-alpha.3; desplegar el recolector de reportes en la web.
3. **Fused**: el host Bukkit real dentro del servidor integrado (hoy solo prepara el entorno). Después, builds por loader (NeoForge/Forge) si se necesitan.
4. **Horizon** (cliente Rust 26.3) como motor de una instancia, cuando exista el client ID propio.
5. ~~Servidor local gestionado~~ (hecho, sin publicar: ver CHANGELOG). Falta probar la UI real con jugador y un selector LAN en la interfaz.
6. Importar instancias de Prism/CurseForge/Modrinth; modpacks; perfil de memoria automático según la RAM del equipo.
7. Decidir el destino de los crates heredados (`drakestar-app`, `-core`, `-cli`, `-rust-client`, `-native-example`).

## Historial (diseño anterior, v0.1 a v0.5.2)

Perfiles, servidor local DrakesPurpur, intermediario/hooks nativos, análisis de mods, cliente Rust propio y vistas de Horizon: ver el historial de git (`git log v0.5.2`) y los crates heredados. La interfaz nueva no los expone.
