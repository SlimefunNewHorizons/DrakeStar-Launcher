<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { brand, modeLabel, modeHome } from '../lib/brand';
  import { sound } from '../lib/sound';
  import { router } from '../lib/router.svelte';
  import ProfileSummary from '../components/ProfileSummary.svelte';
  import StatePill from '../components/StatePill.svelte';
  import CyberHud from '../components/CyberHud.svelte';
  import DallasCard from '../components/DallasCard.svelte';
  import type { JavaCheck, ServerStatus, Profile } from '../lib/types';

  const p = $derived(app.active);
  const compat = $derived(p ? app.compatOf(p.mode) : undefined);
  let java = $state<JavaCheck | null>(null);
  let srv = $state<ServerStatus | null>(null);
  let launching = $state(false);

  $effect(() => {
    java = null; srv = null;
    const cur = p;
    if (cur?.mode === 'local-server') {
      void api.javaCheck(cur.id).then((j) => (java = j)).catch(() => {});
      void api.serverStatus(cur.id).then((s) => (srv = s)).catch(() => {});
    }
  });

  async function playActive() {
    if (!p) return;
    sound.launch();
    launching = true;
    try {
      if (p.mode === 'drakes-rust') {
        let st = await api.rustStatus(p.id).catch(() => null);
        if (!st || !st.ready) {
          app.notify('info', 'Instalando y verificando cliente nativo DrakesRust...');
          st = await api.rustInstall(p.id);
        }
        await api.rustStart(p.id);
        sound.success();
        app.notify('ok', `⚔️ ¡Cliente DrakesRust conectado a ${p.server || 'mc.drakescraft.cl:25565'}!`);
        return;
      }

      if (p.mode === 'drakes-java') {
        app.notify('info', `Comprobando y lanzando cliente Minecraft Java (${p.loader})...`);
        let cst = await api.clientStatus(p.id).catch(() => null);
        if (!cst || !cst.ready) {
          app.notify('info', 'Descargando artefactos de Minecraft y Fabric Loader...');
          await api.clientInstall(p.id);
        }
        await api.clientStart(p.id);
        sound.success();
        app.notify('ok', `🎮 ¡Proceso de Minecraft iniciado con éxito (${p.name})!`);
        router.go('/instances');
        return;
      }

      if (p.mode === 'local-server') {
        const r = await app.guard(() => api.serverStart(p.id), 'Servidor arrancando…');
        if (r) {
          sound.success();
          router.go('/forge/console');
        }
        return;
      }

      if (!compat?.launch_implemented) {
        app.notify('info', compat?.launch_note ?? 'Lanzamiento no disponible');
      }
    } catch (err: any) {
      sound.error();
      app.notify('err', `Error al iniciar: ${err?.message || err}`);
    } finally {
      setTimeout(() => { launching = false; }, 2000);
    }
  }

  async function quickLaunchLan() {
    let localProf = app.profiles.find((pr) => pr.mode === 'local-server');
    if (!localProf) {
      const newProf: Profile = {
        schema: 1,
        id: 'purpur-slimefun-lan',
        name: 'Purpur Slimefun LAN',
        mode: 'local-server',
        channel: 'stable',
        minecraft: '26.2',
        loader: 'vanilla',
        java: {
          major: 25,
          min_mem_mb: 2048,
          max_mem_mb: 4096,
          extra_jvm_args: ['-XX:+UseG1GC', '-Dfile.encoding=UTF-8'],
        },
        server: 'drakespurpur.jar',
        preset: 'slimefun-classic',
        modpack: null,
        notes: 'Instancia LAN Solitario/Multijugador local con Slimefun v11 + StarSuites',
      };
      await app.guard(() => api.createProfile(newProf));
      await api.acceptEula(newProf.id).catch(() => {});
      localProf = newProf;
    }
    const profId = localProf.id;
    await app.setActive(profId);

    // Comprobar si el JAR del servidor ya está instalado
    const srv = await api.serverStatus(profId).catch(() => null);
    if (!srv?.jar?.selected) {
      app.notify('info', 'Descargando y verificando motor DrakesPurpur 26.2 (SHA-256)...');
      await app.guard(() => api.installServerJar(profId), 'Motor DrakesPurpur instalado con éxito');
    }

    // Asegurar EULA aceptada
    await api.acceptEula(profId).catch(() => {});

    app.notify('ok', '⚡ Instancia Purpur + StarSuites iniciada. Abriendo consola Blackwall...');
    const r = await app.guard(() => api.serverStart(profId), 'Servidor local iniciado');
    if (r) {
      router.go('/forge/console');
    } else {
      router.go('/forge');
    }
  }

  async function launchMinecraftClient() {
    // Buscar o crear perfil Fabric de cliente
    let clientProf = app.profiles.find((pr) => pr.mode === 'drakes-java');
    if (!clientProf) {
      clientProf = {
        schema: 1,
        id: 'drakes-fabric-client',
        name: 'Minecraft Fabric (DrakeStar Fused)',
        mode: 'drakes-java',
        channel: 'stable',
        minecraft: '1.21.1',
        loader: 'fabric',
        java: {
          major: 21,
          min_mem_mb: 2048,
          max_mem_mb: 4096,
          extra_jvm_args: ['-XX:+UseG1GC'],
        },
        graphics: {
          backend: 'opengl',
          width: 1280,
          height: 720,
          fullscreen: false,
          discrete_gpu: true,
        },
        server: 'mc.drakescraft.cl:25565',
        preset: null,
        modpack: null,
        notes: 'Cliente Minecraft con Fabric y shaders compatibles',
      };
      await app.guard(() => api.createProfile(clientProf!));
    }
    await app.setActive(clientProf.id);
    await playActive();
  }
</script>

<div class="row between">
  <h1>Centro de Mando DrakeStar</h1>
</div>

<CyberHud />

<div class="home-layout-stack">
  <!-- 1. HERO VIEW PRINCIPAL: SLIMEFUN EN SOLITARIO / LAN (Llegar y Jugar) -->
  <section class="card stack hero-slimefun">
    <header class="hero-header">
      <div class="hero-icon-container">
        <span class="hero-glow-icon">⚡</span>
      </div>
      <div class="t">
        <div class="row items-center gap-sm">
          <h2 class="hero-title">MODALIDAD SLIMEFUN SOLITARIO / LAN</h2>
          <span class="badge hero-badge">⭐ HERO · LLEGAR Y JUGAR</span>
        </div>
        <span class="hero-subtitle">
          Motor DrakesPurpur 26.2 × StarSuites (20,000+ Items Slimefun v11) en una sola instancia interna
        </span>
      </div>
    </header>

    <div class="hero-body">
      <p class="hero-description">
        Juega toda la experiencia técnica, mágica y nuclear de Slimefun directamente en tu máquina sin configurar nada. 
        Compatible con conexión local individual (Solitario) o apertura LAN para jugar con tus amigos por WiFi o Tailscale.
      </p>

      <div class="instruction-box">
        <div class="instruction-header">
          <span class="book-icon">📖</span>
          <strong>INSTRUCCIONES DE ACCESO RÁPIDO:</strong>
        </div>
        <ol class="instructions-list">
          <li>
            Pulsa <strong>▶ INICIAR SLIMEFUN LOCAL</strong>. El launcher instala y valida automáticamente <code>drakespurpur.jar</code> (SHA-256) con las suites oficiales.
          </li>
          <li>
            Se desplegará la <strong>Consola Blackwall</strong> para monitorear TPS, memoria, advertencias y bucles del servidor integrado en tiempo real.
          </li>
          <li>
            En tu Minecraft (Java o Fabric con shaders Iris/Sodium), únete a <code>localhost:25565</code> o <code>127.0.0.1:25565</code>.
          </li>
          <li>
            ¡Listo para jugar! Usa <code>/sf guide</code> in-game para acceder a las 20,000+ recetas tecnológicas.
          </li>
        </ol>
      </div>

      <div class="ecosystem-links-box">
        <span class="muted small-text">📦 ECOSISTEMA OPEN SOURCE:</span>
        <div class="row wrap gap-sm">
          <a class="tag-link" href="https://github.com/SlimefunNewHorizons" target="_blank" rel="noopener noreferrer">
            🏛 GitHub @SlimefunNewHorizons
          </a>
          <a class="tag-link" href="https://github.com/SlimefunNewHorizons/Drakes-Suites" target="_blank" rel="noopener noreferrer">
            ⚙️ Repositorio StarSuites Monorepo
          </a>
          <a class="tag-link" href="https://github.com/SlimefunNewHorizons/DrakeStar-Launcher" target="_blank" rel="noopener noreferrer">
            🚀 DrakeStar Launcher Repo
          </a>
          <a class="tag-link" href="https://github.com/SlimefunNewHorizons/drakestar-fused" target="_blank" rel="noopener noreferrer">
            ⚡ DrakeStar Fused Bridge Mod
          </a>
        </div>
      </div>
    </div>

    <footer class="hero-footer row between wrap">
      <div class="row">
        <button class="btn primary hero-play-btn" onclick={quickLaunchLan}>
          ▶ INICIAR SLIMEFUN LOCAL + CONSOLA
        </button>
        <button class="btn" onclick={launchMinecraftClient} style="border: 1px solid #10b981; color: #10b981; font-weight: bold;">
          🎮 LANZAR MINECRAFT (CLIENTE JAVA)
        </button>
        <button class="btn" onclick={() => router.go('/forge/plugins')}>
          📁 Abrir Plugins
        </button>
        <button class="btn" onclick={() => router.go('/forge/console')}>
          🖥️ Consola Flotante
        </button>
      </div>

      <div class="row">
        <span class="status-indicator-inline">
          <span class="dot-live"></span>
          FFM ABI 1 + SIMD Off-Heap Ready
        </span>
      </div>
    </footer>
  </section>

  <!-- 2. SERVIDOR REMOTO DALLAS (SMP TEMPORADA 2) -->
  <DallasCard />

  <!-- 3. PERFIL ACTIVO Y EXPERIMENTAL / MODS -->
  <div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(320px,1fr))">
    {#if p}
      <section class="card stack" aria-labelledby="h-active">
        <header>
          <img class="mi" src={brand.modeIcon(p.mode)} alt="" />
          <div class="t"><h2 id="h-active" style="margin:0">{p.name}</h2><span class="muted mono">{p.id}</span></div>
          {#if p.mode === 'local-server' || p.mode === 'drakes-java'}<StatePill state={app.statusOf(p.id).state} />{/if}
        </header>
        <ProfileSummary {p} />
        <div class="row">
          <button class="play" onclick={playActive} aria-label="Lanzar perfil activo" style="cursor: pointer;">
            <img src="./brand/ui/button-play-primary.svg" alt="JUGAR" height="52" />
          </button>
          <button class="btn" onclick={() => router.go(modeHome[p.mode])}>Abrir {modeLabel[p.mode]}</button>
        </div>
        {#if p.mode === 'drakes-rust'}
          <div class="alert ok" style="border-left: 3px solid var(--gold); background: rgba(245, 185, 66, 0.08); color: var(--gold-2); font-size: 0.9rem;">
            ⚔️ <strong>Servidor Destino:</strong> mc.drakescraft.cl:25565 · Rust Native Client
          </div>
        {:else if p.mode === 'drakes-java'}
          <div class="alert ok" style="border-left: 3px solid #10b981; background: rgba(16, 185, 129, 0.08); color: #34d399; font-size: 0.9rem;">
            🎮 <strong>Cliente Minecraft Java:</strong> Loader {p.loader} · KnotClient Activo
          </div>
        {:else if compat && !compat.launch_implemented}
          <div class="alert warn">Lanzar no está disponible para {modeLabel[p.mode]}: {compat.launch_note}.</div>
        {/if}
      </section>

      {#if p.mode === 'local-server'}
        <section class="card stack" aria-labelledby="h-comp">
          <h2 id="h-comp">Estado de componentes de {p.name}</h2>
          <dl class="kv">
            <dt>Java</dt>
            <dd>{#if !java}comprobando…{:else if java.info.found}<span class="badge yes">{java.info.version}</span>{:else}<span class="badge no">no encontrado</span>{/if}</dd>
            <dt>JAR del servidor</dt>
            <dd>{#if !srv}…{:else if srv.jar.selected}<span class="badge yes">{srv.jar.selected}</span>{:else}<span class="badge no">falta</span>{/if}</dd>
            <dt>EULA</dt>
            <dd>{#if !srv}…{:else if srv.eula}<span class="badge yes">aceptada</span>{:else}<span class="badge planned">sin aceptar</span>{/if}</dd>
            <dt>Plugins activos</dt><dd>{srv?.plugins ?? '…'}</dd>
            <dt>Mundos</dt><dd>{srv?.worlds ?? '…'}</dd>
          </dl>
          {#if java?.warning}<div class="alert warn">{java.warning}</div>{/if}
        </section>
      {/if}
    {/if}

    <!-- Roadmap & Guía de Motores -->
    <section class="card stack">
      <h2>Arquitectura Multi-Motor DrakeStar</h2>
      <ul class="muted" style="padding-left:1.1rem;margin:0; font-size: 0.85rem; line-height: 1.5;">
        <li><strong>Slimefun Solitario / LAN:</strong> Purpur 26.2 + StarSuites 20k+ items sin lag.</li>
        <li><strong>Cliente Minecraft Java (Fabric):</strong> KnotClient integrado con mods y shaders.</li>
        <li><strong>DrakesRust Native Client:</strong> Protocolo 767, conexión C/Rust directa sin JVM.</li>
        <li><strong>Bridge Mod (DrakeStar Fused):</strong> Inyección de Bukkit en el IntegratedServer.</li>
        <li><strong>Discord SRE & Soporte:</strong> Integración 24/7 con SAORI para soporte y reportes.</li>
      </ul>
      <div class="row" style="margin-top:0.4rem;">
        <button class="btn small" onclick={() => router.go('/profiles')}>Gestionar Perfiles</button>
        <button class="btn small" onclick={() => router.go('/settings')}>Ajustes del Launcher</button>
      </div>
    </section>

    <!-- Discord Card -->
    <section class="card stack" style="border: 1px solid rgba(88, 101, 242, 0.4); background: linear-gradient(135deg, rgba(18, 20, 38, 0.9), rgba(12, 14, 28, 0.95));">
      <header>
        <img src={brand.icon('discord')} alt="Discord" width="28" height="28" />
        <div class="t">
          <h2 style="margin:0; color:#9fa8ff; font-size:1.05rem;">COMUNIDAD DRAKESGRAFT</h2>
          <span class="muted" style="font-size:0.75rem;">Soporte, anuncios, guías e integración SRE</span>
        </div>
      </header>
      <p style="font-size:0.85rem; color:#8b949e; margin:0.25rem 0; line-height:1.4;">
        Únete a nuestro Discord oficial soberano. Accede a canales bilingües, avisos de mantenimiento, changelogs en vivo y asistencia directa de SAORI.
      </p>
      <div class="row">
        <a class="btn primary" href={brand.discordUrl} target="_blank" rel="noopener noreferrer" style="background: linear-gradient(135deg, #5865f2, #4752c4); color:#fff; border:none; font-weight:bold; text-decoration:none; display:inline-flex; align-items:center; gap:0.5rem; font-size:0.85rem;">
          <img src={brand.icon('discord')} alt="" width="16" height="16" />
          UNIRSE A DISCORD
        </a>
      </div>
    </section>
  </div>
</div>

<style>
  .home-layout-stack {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .hero-slimefun {
    border: 1px solid rgba(0, 240, 255, 0.45);
    background: linear-gradient(135deg, rgba(6, 18, 30, 0.92) 0%, rgba(2, 8, 16, 0.96) 100%);
    box-shadow: 0 4px 28px rgba(0, 240, 255, 0.12), inset 0 0 16px rgba(0, 240, 255, 0.04);
  }

  .hero-header {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .hero-icon-container {
    width: 48px;
    height: 48px;
    border-radius: 10px;
    background: rgba(0, 240, 255, 0.1);
    border: 1px solid rgba(0, 240, 255, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .hero-glow-icon {
    font-size: 1.8rem;
    text-shadow: 0 0 12px #00f0ff;
  }

  .hero-title {
    margin: 0;
    color: #00f0ff;
    font-size: 1.2rem;
    font-family: monospace;
    letter-spacing: 0.04em;
  }

  .hero-badge {
    background: rgba(0, 240, 255, 0.15);
    color: #00f0ff;
    border: 1px solid rgba(0, 240, 255, 0.3);
    font-weight: bold;
    font-size: 0.7rem;
    letter-spacing: 0.06em;
  }

  .hero-subtitle {
    font-size: 0.82rem;
    color: #94a3b8;
    display: block;
    margin-top: 0.15rem;
  }

  .hero-body {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .hero-description {
    font-size: 0.9rem;
    color: #cbd5e1;
    margin: 0;
    line-height: 1.5;
  }

  .instruction-box {
    background: rgba(0, 0, 0, 0.45);
    border: 1px dashed rgba(0, 240, 255, 0.3);
    border-radius: 8px;
    padding: 0.85rem;
    font-size: 0.82rem;
    color: #e2e8f0;
  }

  .instruction-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: #00f0ff;
    font-size: 0.85rem;
    margin-bottom: 0.4rem;
  }

  .instructions-list {
    margin: 0 0 0 1.25rem;
    padding: 0;
    line-height: 1.6;
  }

  .instructions-list code {
    background: rgba(0, 240, 255, 0.12);
    color: #38bdf8;
    padding: 0.1rem 0.35rem;
    border-radius: 4px;
    font-family: monospace;
  }

  .ecosystem-links-box {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .small-text {
    font-size: 0.7rem;
    letter-spacing: 0.05em;
  }

  .tag-link {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #94a3b8;
    text-decoration: none;
    font-size: 0.75rem;
    padding: 0.3rem 0.6rem;
    border-radius: 5px;
    transition: all 0.2s ease;
  }

  .tag-link:hover {
    background: rgba(0, 240, 255, 0.12);
    border-color: rgba(0, 240, 255, 0.3);
    color: #00f0ff;
  }

  .hero-footer {
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .hero-play-btn {
    background: linear-gradient(135deg, #00f0ff 0%, #0284c7 100%);
    border: none;
    font-weight: bold;
    color: #030712;
    letter-spacing: 0.04em;
    padding: 0.6rem 1.25rem;
    box-shadow: 0 0 16px rgba(0, 240, 255, 0.35);
  }

  .status-indicator-inline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: #38bdf8;
    font-family: monospace;
  }

  .dot-live {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #00f0ff;
    box-shadow: 0 0 8px #00f0ff;
    animation: pulse-glow 2s infinite;
  }

  @keyframes pulse-glow {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.5; transform: scale(0.85); }
  }
</style>
