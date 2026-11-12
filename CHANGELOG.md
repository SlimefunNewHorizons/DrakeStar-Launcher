# Cambios

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
