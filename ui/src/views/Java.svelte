<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { router } from '../lib/router.svelte';
  import ProfileSummary from '../components/ProfileSummary.svelte';
  import CompatTable from '../components/CompatTable.svelte';
  import type { JavaCheck, Profile } from '../lib/types';
  let { p }: { p: Profile } = $props();
  let java = $state<JavaCheck | null>(null);
  $effect(() => { java = null; void app.guard(() => api.javaCheck(p.id)).then((j) => (java = j ?? null)); });
</script>

<h1>DrakesJava — {p.name}</h1>
<div class="alert ok" style="border-left: 3px solid #10b981; background: rgba(16, 185, 129, 0.08); color: #34d399;">
  🎮 <strong>Cliente Minecraft Java (Fabric) Activo:</strong> Descarga automatizada de librerías, renderizado OpenGL / Vulkan, modo pantalla completa y enlace directo a servidores.
</div>
<div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(340px,1fr))">
  <section class="card stack"><h2>Perfil</h2><ProfileSummary {p} />
    <div class="row" style="gap: 0.5rem; margin-top: 0.5rem;">
      <button class="btn primary" onclick={async () => {
        app.notify('info', `Comprobando y lanzando cliente Minecraft Java (${p.loader})...`);
        let st = await api.clientStatus(p.id).catch(() => null);
        if (!st || !st.ready) {
          app.notify('info', 'Descargando artefactos de Minecraft y Fabric...');
          await api.clientInstall(p.id);
        }
        await api.clientStart(p.id);
        app.notify('ok', `🎮 ¡Proceso de Minecraft iniciado (${p.name})!`);
        router.go('/instances');
      }}>🎮 Lanzar Minecraft</button>
      <button class="btn" onclick={() => router.go('/profiles')}>Editar Perfil</button>
    </div>
  </section>

  <section class="card stack"><h2>Java del sistema</h2>
    {#if !java}<p class="muted">Comprobando…</p>
    {:else}
      <dl class="kv"><dt>Ejecutable</dt><dd><code>{java.info.bin}</code></dd><dt>Versión</dt><dd>{java.info.version ?? 'no detectada'}</dd></dl>
      {#if java.warning}<div class="alert warn">{java.warning}</div>{:else}<div class="alert info">Java suficiente para Minecraft {p.minecraft}.</div>{/if}
    {/if}</section>
  <section class="card stack"><h2>Compatibilidades</h2><CompatTable mode="drakes-java" /></section>
</div>
