# DrakeStar

Launcher de Minecraft de **DrakesCraft**. Un solo botón, **JUGAR**: instala Java, el juego y los mods por ti y abre Minecraft **1.20 a 26.2** con tu cuenta de Microsoft o sin premium.

[English](README.md)

## Instalar

Descarga la última versión desde [Releases](https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/latest):

| Sistema | Instalador | Nota |
|---|---|---|
| Windows | `.exe` (recomendado) o `.msi` | el `.exe` instala por usuario y se actualiza sin permisos de administrador; el `.msi` instala para todo el equipo y pide administrador al actualizar |
| Linux | `.deb`, `.rpm`, `.AppImage` | |
| macOS | `.dmg` | |

DrakeStar se **actualiza solo**: busca versiones nuevas al abrir y desde *Ajustes → Buscar actualizaciones* (las versiones vienen firmadas).

## Usar

1. **Cuentas**: inicia sesión con Microsoft (se abre tu navegador solo; solo escribes el código si te lo pide) o agrega una cuenta sin premium. Puedes tener varias de cada tipo y elegir con cuál jugar.
2. **Elige qué jugar**: *DrakesCraft (online)*, *Slimefun en solitario / LAN*, *Minecraft vanilla* o una instancia tuya (versión, Fabric o vanilla, memoria, mods de rendimiento).
3. **JUGAR**. La primera vez descarga Java y los archivos del juego; después abre en segundos.

Fabric instala además Fabric API y, si lo activas, los mods de rendimiento (Sodium, Lithium, FerriteCore, ImmediatelyFast, Entity Culling) y **DrakeStar Fused** (alfa: un solo `.jar` para 1.20 a 26.2).

## Registros y privacidad

- El launcher guarda sus registros (14 días) y los del juego en la carpeta de datos (*Ajustes → Abrir carpeta de registros*). Si algo falla, *Registros* lo muestra y permite crear/enviar un reporte.
- Si el juego se cierra con error, DrakeStar envía **automáticamente** un reporte **solo para arreglar fallos** (se puede desactivar en *Ajustes → Privacidad*; se explica en la primera apertura). Contiene: versión de DrakeStar y de Minecraft, sistema operativo, nombres de mods y las últimas líneas de los registros **con rutas, correos, IPs y claves tachados**. Nunca contraseñas ni tokens. Cerrar el juego tú mismo no cuenta como fallo.
- Los tokens de Microsoft se guardan aparte de la lista de cuentas (`accounts/secrets.json`, solo tu usuario) y nunca se escriben en los registros.

## Documentación

[Novedades](CHANGELOG.md) · [Guía de operación](docs/LAUNCHER.md) (cuentas y Azure, reportes, actualizaciones, publicación, Fused, Horizon) · [Arquitectura](docs/ARCHITECTURE.md) · [Roadmap](docs/ROADMAP.md) · [Marca](docs/BRANDING.md)

## Desarrollo

Requisitos: Rust estable, Node 22+, y en Linux las dependencias de Tauri 2 (`libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config`).

```bash
cd ui && npm install            # una vez
npm run dev                     # la interfaz en el navegador con datos de ejemplo (?fresh = primer arranque, ?update = versión nueva)
cd .. && ui/node_modules/.bin/tauri dev      # la app completa
cargo test -p drakestar-mc      # motor (descargas, Java, lanzamiento, cuentas, registros)
(cd ui && npm run build)        # svelte-check + vite build
cargo run -p drakestar-mc --example play_test -- 1.21.11 vanilla 60   # prueba real de lanzamiento
```

`DRAKESTAR_DATA_DIR` cambia la carpeta de datos. Detalle de pruebas de punta a punta con Xvfb en [docs/LAUNCHER.md](docs/LAUNCHER.md#pruebas-de-punta-a-punta).

## Licencia

MIT. Creado por JackStar6677 ([SlimefunNewHorizons](https://github.com/SlimefunNewHorizons)). Inspirado en [Horizon](https://github.com/horizon-rust/horizon) (MIT); este repo no copia su código ni su marca.
