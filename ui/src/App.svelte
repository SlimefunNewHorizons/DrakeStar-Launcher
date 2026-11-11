<script lang="ts">
  import { onMount, type Component } from 'svelte';
  import { app } from './lib/state.svelte';
  import { router, routes, type RouteDef } from './lib/router.svelte';
  import { brand, modeHome, modeLabel } from './lib/brand';
  import { sound } from './lib/sound';
  import type { Mode, Profile } from './lib/types';
  import Gate from './components/Gate.svelte';
  import Home from './views/Home.svelte';
  import Profiles from './views/Profiles.svelte';
  import Instances from './views/Instances.svelte';
  import Rust from './views/Rust.svelte';
  import Java from './views/Java.svelte';
  import Modpacks from './views/Modpacks.svelte';
  import Forge from './views/Forge.svelte';
  import Console from './views/Console.svelte';
  import Plugins from './views/Plugins.svelte';
  import Worlds from './views/Worlds.svelte';
  import Online from './views/Online.svelte';
  import Updates from './views/Updates.svelte';
  import About from './views/About.svelte';
  import CrashReporterModal from './components/CrashReporterModal.svelte';
  import MicrosoftAuthModal from './components/MicrosoftAuthModal.svelte';

  let fatal = $state<string | null>(null);
  let sidebarOpen = $state(false);
  let msftModal = $state<{ openModal: () => void; closeModal: () => void } | undefined>();
  onMount(() => { app.init().catch((e) => (fatal = e instanceof Error ? e.message : String(e))); });

  const views: Record<string, Component<any>> = {
    '/': Home, '/profiles': Profiles, '/instances': Instances, '/rust': Rust, '/java': Java, '/java/modpacks': Modpacks,
    '/forge': Forge, '/forge/console': Console, '/forge/console-detached': Console, '/forge/plugins': Plugins, '/forge/worlds': Worlds,
    '/online': Online, '/settings/updates': Updates, '/settings/about': About,
  };
  const icons: Record<string, string> = {
    '/': brand.isotype, '/profiles': brand.icon('profile'), '/instances': brand.icon('world'), '/settings/updates': brand.icon('update'),
    '/settings/about': brand.icon('settings'), '/forge/console': brand.icon('console'), '/forge/console-detached': brand.icon('console'),
    '/forge/plugins': brand.icon('plugin'), '/forge/worlds': brand.icon('world'), '/java/modpacks': brand.icon('modpack'),
  };
  const modes: Mode[] = ['drakes-rust', 'drakes-java', 'local-server', 'online'];

  const active = $derived(app.active);
  const def = $derived(router.def);
  const View = $derived(views[def.path] ?? Home);
  const gated = $derived(!!def.mode && active?.mode !== def.mode);
  const general = routes.filter((r) => !r.mode && !r.path.startsWith('/settings'));
  const settingsRoutes = routes.filter((r) => r.path.startsWith('/settings'));
  const modeRoutes = $derived(active ? routes.filter((r) => r.mode === active.mode) : []);

  function pickMode(m: Mode) {
    const cur = app.active?.mode === m ? app.active : app.profiles.find((p) => p.mode === m);
    if (cur && cur.id !== app.settings.active_profile) void app.setActive(cur.id);
    router.go(cur ? modeHome[m] : '/profiles');
    if (!cur) app.notify('info', `No tienes perfiles de ${modeLabel[m]}: crea uno.`);
  }
  async function onSelect(e: Event) { await app.setActive((e.currentTarget as HTMLSelectElement).value || null); }
  $effect(() => { document.title = `${def.title} · DrakeStar`; });
</script>

<a class="skip" href="#main">Saltar al contenido</a>
<div class="root">
  {#if router.path === '/forge/console-detached'}
    <div class="detached-console-wrapper" style="min-height:100vh;background:#05070a;padding:1.5rem;box-sizing:border-box;overflow-y:auto">
      <Console p={active as Profile} detached={true} />
    </div>
  {:else}
    <div class="cyber-header-bar" role="status">
      <span class="live-dot"></span>
      <span class="title">DRAKESTAR DECK v{app.info?.version ?? '0.5.1'}</span>
      <span class="divider">::</span>
      <span class="target">ONLINE SMP · mc.drakescraft.cl:25565 (T2)</span>
      <span class="divider">::</span>
      <span class="kernel">PURPUR 26.2 × RUST FFM ENGINE</span>
    </div>
    <header class="topbar">
      <button
        type="button"
        class="btn icon-btn mobile-menu-toggle"
        onclick={() => { sound.click(); sidebarOpen = !sidebarOpen; }}
        aria-label="Alternar menú lateral"
        aria-expanded={sidebarOpen}
      >
        <span style="font-size:1.15rem; line-height:1;">{sidebarOpen ? '✕' : '☰'}</span>
      </button>
      <a class="brand" href="#/" aria-label="DrakeStar, inicio" onclick={() => (sidebarOpen = false)}>
        <img src={brand.logo} alt="DrakeStar" />
      </a>
      <div class="row items-center" style="gap:.75rem;margin-left:auto">
        <div role="group" aria-label="Selector de modo" class="tabs">
          {#each modes as m (m)}
            <button onclick={() => { sound.click(); pickMode(m); sidebarOpen = false; }} onmouseenter={() => sound.hover()} aria-pressed={active?.mode === m} aria-label={modeLabel[m]}><img src={brand.tab(m, active?.mode === m)} alt={modeLabel[m]} /></button>
          {/each}
        </div>
        <button
          type="button"
          class="btn small"
          style="display:inline-flex;align-items:center;gap:.4rem;padding:.35rem .75rem;border-radius:4px;font-size:0.8rem;background:rgba(255,255,255,0.06);border:1px solid rgba(255,255,255,0.12)"
          onclick={() => msftModal?.openModal()}
          title={app.settings.session_name ? `Conectado como ${app.settings.session_name}` : 'Vincular cuenta de Minecraft / Microsoft'}
        >
          <span>🟩</span>
          {#if app.settings.session_name}
            <span style="max-width:120px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:600">
              {app.settings.session_name}
            </span>
            <span class="badge yes" style="font-size:0.65rem;padding:1px 4px">MSA</span>
          {:else}
            <span>Login MS</span>
          {/if}
        </button>
      </div>
    </header>
    <div class="body">
      {#if sidebarOpen}
        <button
          type="button"
          class="sidebar-backdrop"
          onclick={() => (sidebarOpen = false)}
          aria-label="Cerrar menú lateral"
        ></button>
      {/if}
      <aside class="sidebar" class:open={sidebarOpen}>
        <div class="field" style="margin:0">
          <label for="active-profile">Perfil activo</label>
          <select id="active-profile" value={app.settings.active_profile ?? ''} onchange={(e) => { sound.click(); onSelect(e); sidebarOpen = false; }}>
            {#if !app.profiles.length}<option value="">(sin perfiles)</option>{/if}
            {#each app.profiles as p (p.id)}<option value={p.id}>{p.name} · {p.channel}</option>{/each}
          </select>
        </div>
        <nav aria-label="Navegación principal">
          <h3>General</h3>
          {#each general as r (r.path)}
            <a class="navlink" href={`#${r.path}`} onclick={() => { sound.click(); sidebarOpen = false; }} onmouseenter={() => sound.hover()} aria-current={router.path === r.path ? 'page' : undefined}><img src={icons[r.path] ?? brand.isotype} alt="" />{r.title}</a>
          {/each}
          {#if active}
            <h3>{modeLabel[active.mode]}</h3>
            {#each modeRoutes as r (r.path)}
              <a class="navlink" class:sub={r.path.split('/').length > 2} href={`#${r.path}`} onclick={() => { sound.click(); sidebarOpen = false; }} onmouseenter={() => sound.hover()} aria-current={router.path === r.path ? 'page' : undefined}><img src={icons[r.path] ?? brand.modeIcon(active.mode)} alt="" />{r.title}</a>
            {/each}
          {/if}
          <h3>Ajustes</h3>
          {#each settingsRoutes as r (r.path)}
            <a class="navlink" href={`#${r.path}`} onclick={() => { sound.click(); sidebarOpen = false; }} onmouseenter={() => sound.hover()} aria-current={router.path === r.path ? 'page' : undefined}><img src={icons[r.path] ?? brand.icon('settings')} alt="" />{r.title}</a>
          {/each}
          <h3>Comunidad</h3>
          <a class="navlink" href={brand.discordUrl} target="_blank" rel="noopener noreferrer" onclick={() => { sound.click(); sidebarOpen = false; }} onmouseenter={() => sound.hover()} title="Unirse al Discord oficial de DrakesCraft">
            <img src={brand.icon('discord')} alt="Discord" />
            Discord Oficial
          </a>
        </nav>
      </aside>
      <main id="main" tabindex="-1">
        {#if fatal}
          <div class="alert err" role="alert">No se pudo iniciar la interfaz: {fatal}</div>
        {:else if !app.ready}
          <p class="muted" role="status">Cargando…</p>
        {:else if gated && def.mode}
          <Gate mode={def.mode} />
        {:else}
          {#key def.path + (active?.id ?? '')}
            <View p={active as Profile} />
          {/key}
        {/if}
      </main>
    </div>
  {/if}
  {#if app.toast}
    <div class="toast {app.toast.kind}" role={app.toast.kind === 'err' ? 'alert' : 'status'}>{app.toast.text}</div>
  {/if}
  <CrashReporterModal
    bind:show={app.crashReport.show}
    title={app.crashReport.title}
    errorLog={app.crashReport.errorLog}
    component={app.crashReport.component}
  />
  <MicrosoftAuthModal bind:this={msftModal} />
</div>