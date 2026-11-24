# DrakeStar (`v0.7.0` — Feature-Complete)

> **Status:** `Feature-Complete / Production Freeze (v0.7.0)` — Stable production launcher with automated signed CI/CD releases (`--locked` + minisign).

The **DrakesCraft** Minecraft launcher. One **PLAY** button with **Dual Engine Mode** (**Java Aikar G1GC** + native **DrakesRust / Horizon 26.3**) and **Local Monolith Mode** (*Slimefun single-player / LAN* powered by DrakesPurpur 26.2 in ~22s with clean shutdown). Opens Minecraft **1.20 to 26.2** with a Microsoft or offline account.

[Español](README_ES.md)

## Install

Get the latest build from [Releases](https://github.com/SlimefunNewHorizons/DrakeStar-Launcher/releases/latest):

| OS | Installer | Note |
|---|---|---|
| Windows | `.exe` (recommended) or `.msi` | the `.exe` installs per user and updates without admin rights; the `.msi` is per machine and asks for admin to update |
| Linux | `.deb`, `.rpm`, `.AppImage` | |
| macOS | `.dmg` | |

DrakeStar **updates itself** (signed releases): it checks on start and from *Settings → Check for updates*.

## Use

1. **Accounts**: sign in with Microsoft (your browser opens by itself; type the code only if asked) or add an offline account. Keep as many of each as you want and pick which one to play with.
2. **Pick what to play**: *DrakesCraft (online)*, *Slimefun single-player / LAN* (in 26.2 + Java 25 with **DrakeStar Fused** `v1.0.0-alpha.4+`, embeds the **DrakesPurpur 26.2** monolith with Slimefun + 9 StarSuites + Rust FFM/JNI directly inside the Fabric client JVM and joins it without a second server process; ~6 GB free RAM, falling back to the managed local server if Fused is disabled), *Vanilla* or your own instance (version, Fabric or vanilla, memory, performance mods).
3. **PLAY**. The first time it downloads Java and the game files; afterwards it opens in seconds.
4. **Native engine** (*Native engine*, next to PLAY): the Rust engine — Horizon for Minecraft 26.3. It installs Horizon's own binaries (from Windows x64/arm64, Linux and macOS), prepares them with a managed JDK on first run, and joins your server with the Microsoft account if you pick one.

Fabric instances also get Fabric API and, if enabled, the performance mods (Sodium, Lithium, FerriteCore, ImmediatelyFast, Entity Culling) and **DrakeStar Fused** (alpha: one `.jar` for 1.20 to 26.2).

**Releasing**: never build by hand. `Cargo.toml`, `src-tauri/tauri.conf.json` and `ui/package.json` carry the same version; push the `vX.Y.Z` tag and GitHub Actions builds, signs and publishes on Windows/Ubuntu/macOS:

```bash
node tools/bump-release.mjs 0.7.2     # or: npm run bump-release -- 0.7.2
```

Details (forks, PRs, manual dispatch): [docs/PUBLICAR.md](docs/PUBLICAR.md#publicar-una-versión-con-el-ci-no-compiles-a-mano).

## Logs and privacy

- The launcher keeps its own logs (14 days) and the game's in the data folder (*Settings → Open logs folder*). *Logs* shows them and can create/send a report.
- If the game closes with an error, DrakeStar **automatically** sends a report **only to fix bugs** (opt-out in *Settings → Privacy*; explained on first run): DrakeStar and Minecraft versions, OS, mod names and the last log lines **with paths, emails, IPs and keys removed**. Never passwords or tokens. Closing the game yourself is not a failure.
- Microsoft tokens are stored apart from the account list (`accounts/secrets.json`, your user only) and never written to logs.

## Docs

[Changelog](CHANGELOG.md) (Spanish) · [Operations guide](docs/LAUNCHER.md) (accounts and Azure, reports, updates, releasing, Fused, Horizon) · [Architecture](docs/ARCHITECTURE.md) · [Roadmap](docs/ROADMAP.md)

## Development

Needs stable Rust, Node 22+, and on Linux the Tauri 2 libraries (`libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config`).

```bash
cd ui && npm install            # once
npm run dev                     # UI in the browser with sample data (?fresh = first run, ?update = new version)
cd .. && ui/node_modules/.bin/tauri dev      # the full app
cargo test -p drakestar-mc      # engine (downloads, Java, launching, accounts, logs)
(cd ui && npm run build)        # svelte-check + vite build
(cd ui && npx playwright install chromium && npm run test:catalog) # catalog UI races in Chromium
cargo run -p drakestar-mc --example play_test -- 1.21.11 vanilla 60   # real launch test
```

`DRAKESTAR_DATA_DIR` changes the data folder.

The catalog browser tests use synthetic API responses with delayed reads and installs/removals. They check the Svelte UI, including Spanish and English at 375 px; they do not download content or exercise native Tauri IPC.

## License

MIT. Created by JackStar6677 ([SlimefunNewHorizons](https://github.com/SlimefunNewHorizons)). Inspired by [Horizon](https://github.com/horizon-rust/horizon) (MIT); this repo does not copy its code or brand.
