<p align="center"><img src="assets/brand/banner.svg" alt="DrakeStar — Java × Rust Launcher" width="100%"></p>

<p align="center">
  <a href="https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/tag/v0.5.1"><img alt="Release v0.5.1" src="https://img.shields.io/badge/release-v0.5.1-10b981?style=flat-square"></a>
  <img src="assets/brand/ui/status-experimental.svg" alt="experimental" height="26">
  <img src="assets/brand/ui/status-channel-nightly.svg" alt="stable / nightly" height="26">
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-8B3DFF?style=flat-square"></a>
  <a href="https://discord.gg/rv3vtXZTk7"><img alt="Discord" src="https://img.shields.io/badge/Discord-DrakesCraft-5865F2?style=flat-square&logo=discord&logoColor=white"></a>
</p>

# DrakeStar — The Sovereign Java × Rust Hybrid Launcher

[🇬🇧 English](README.md) · [🇪🇸 Español](README_ES.md)

**DrakeStar** is the official desktop launcher and management station for the **DrakesCraft & Star** ecosystem. It delivers a unified, zero-friction station to **play, test, and orchestrate** Minecraft environments across native Rust performance clients, Fabric Java client execution, and single-instance local Purpur servers featuring the 20,000+ items matrix of **StarSuites & Slimefun v11**.

---

## 🌟 Key Highlights in v0.5.1

* **⭐ Hero Mode: Slimefun Solitario / LAN ("Llegar y Jugar"):**
  * Top-level prominent view for zero-configuration local play.
  * Automatic SHA-256 binary validation of `drakespurpur.jar` 26.2, automated EULA acceptance, and live Blackwall console telemetry (TPS, memory, loops, and warnings).
* **🐉 Dallas SMP (Season 2) Live SLP Card:**
  * Real-time query to `mc.drakescraft.cl:25565` displaying latency, live player count, base64 server favicon, formatted MOTD, and active whitelist/maintenance status.
  * Direct connect via **DrakesRust Native Client** (`drakes-rust`).
* **🎮 Direct Minecraft Java Client Launch (Fabric Active):**
  * One-click **`LANZAR MINECRAFT (CLIENTE JAVA)`** invoking `client_start` with Fabric KnotClient.
  * **Cross-Platform Assets Manager (`assets.rs`):** Deterministic candidate resolution prioritizing `DRAKESTAR_ASSETS_DIR`, instance cache, and external valid caches (Windows `%APPDATA%`, macOS, Linux XDG/Flatpak) strictly verified against `indexes/<id>.json`. Automatic Mojang CDN downloader for asset indexes and essential language/audio files.
* **🛡️ SRE Blackwall Guard & Mod Protection:**
  * Real-time bytecode scanner inspecting `.jar` classes for dangerous payloads (reverse shells, suspicious sockets, process injection) with explicit user override flags.
* **💥 Accessible 1-Click Crash Telemetry:**
  * Integrated crash modal with dynamic runtime versioning, telemetry log extraction, and direct bug report generation on GitHub.
* **⚡ Bridge Integration with DrakeStar Fused:**
  * Native synergy with [DrakeStar Fused](https://github.com/SlimefunNewHorizons/drakestar-fused) to inject Bukkit/Purpur runtime inside Minecraft's `IntegratedServer` with zero dual-JVM RAM overhead.

---

## 🎮 Modes Matrix (Honest Matrix via `compat.rs`)

| Mode | What it does | Status |
|---|---|---|
| **Local Forge (Hero)** | Starts a local DrakesPurpur instance with full StarSuites (20,000+ items); Blackwall console, logs, plugins, and worlds. | **Implemented & Live**: Start/stop, console, logs, EULA, properties, plugins, worlds, and backups. |
| **DrakesJava** | Vanilla & Fabric client launcher with full KnotClient execution, dynamic asset resolver, and shaders support. | **Implemented & Live**: Profile management, Fabric KnotClient execution, asset resolution. *(Forge & NeoForge are planned on the roadmap; see `compat.rs`)*. |
| **DrakesRust** | Native Rust client (Horizon-like protocol 767) for ultra-low latency connection directly to Dallas. | **Implemented & Live**: Binary verification, auto-install, and launch plan. |
| **Online (Dallas SMP)** | Live Server List Ping (SLP) telemetry and fast join to `mc.drakescraft.cl:25565`. | **Implemented & Live**: SLP card, live ping, formatted MOTD, and Discord bridge. |

---

## 📦 Downloads & Releases

Binaries and packages are published reproducibly via GitHub Actions:
* **Release:** [DrakeStar Launcher v0.5.1](https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/tag/v0.5.1)
  * Linux (`.deb`, `.AppImage`)
  * Windows (`.msi`, `.exe`)
  * macOS (`.dmg`)
  * Diagnostic CLI (`drakestar-cli`)
  * `SHA256SUMS.txt`

---

## 🛠️ Building from Source on Nova (i7 Tiger Lake)

Following the sovereign hardware rules of node **Nova**, builds are budgeted to avoid freezing KDE Plasma:

```bash
# Compile CLI / Core with restricted thread budget
cargo build -p drakestar-cli --release -j 4

# Run Desktop UI (Vite + Svelte 5)
cd ui && npm install && npm run build
```

---

## 📜 Credits and License

- **Architect & Creator:** JackStar6677-1 (`pablo.elias.miranda.292003@gmail.com`) — Chief Architect of DrakesCraft & Star.
- **Organization:** [SlimefunNewHorizons](https://github.com/SlimefunNewHorizons)
- **License:** MIT License. Conceptually inspired by [Horizon](https://github.com/horizon-rust/horizon) (MIT).
