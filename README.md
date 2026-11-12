# DrakeStar

The **DrakesCraft** Minecraft launcher. One **PLAY** button: it installs Java, the game and the mods for you and opens Minecraft **1.20 to 26.2** with a Microsoft or offline account.

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
2. **Pick what to play**: *DrakesCraft (online)*, *Slimefun single-player / LAN*, *Vanilla* or your own instance (version, Fabric or vanilla, memory, performance mods).
3. **PLAY**. The first time it downloads Java and the game files; afterwards it opens in seconds.

Fabric instances also get Fabric API and, if enabled, the performance mods (Sodium, Lithium, FerriteCore, ImmediatelyFast, Entity Culling) and **DrakeStar Fused** (alpha: one `.jar` for 1.20 to 26.2).

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
cargo run -p drakestar-mc --example play_test -- 1.21.11 vanilla 60   # real launch test
```

`DRAKESTAR_DATA_DIR` changes the data folder.

## License

MIT. Created by JackStar6677 ([SlimefunNewHorizons](https://github.com/SlimefunNewHorizons)). Inspired by [Horizon](https://github.com/horizon-rust/horizon) (MIT); this repo does not copy its code or brand.
