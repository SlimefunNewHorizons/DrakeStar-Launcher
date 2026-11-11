<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { bytes, when } from '../lib/format';
  import type { Profile, WorldInfo } from '../lib/types';
  let { p }: { p: Profile } = $props();
  let list = $state<WorldInfo[]>([]);
  let backingUp = $state<string | null>(null);

  async function loadWorlds() {
    list = (await api.worlds(p.id).catch(() => [])) ?? [];
  }

  $effect(() => {
    void p.id;
    void loadWorlds();
  });

  async function backup(worldName: string) {
    backingUp = worldName;
    try {
      const dest = await api.backupWorld(p.id, worldName);
      app.notify('ok', `Mundo '${worldName}' respaldado en: ${dest}`);
      await loadWorlds();
    } catch (e: any) {
      app.notify('err', `Error respaldando mundo: ${e?.message || e}`);
    } finally {
      backingUp = null;
    }
  }
</script>

<h1>Mundos — {p.name}</h1>
<p class="muted">Gestión de mundos locales generados con Purpur 26.2 y Slimefun. Puedes crear copias de seguridad instantáneas antes de actualizar o probar plugins.</p>
<div class="card tablewrap">
  <table>
    <thead><tr><th scope="col">Mundo</th><th scope="col">Tamaño</th><th scope="col">Modificado</th><th scope="col">Acciones</th></tr></thead>
    <tbody>
      {#each list as w (w.name)}
        <tr>
          <td><strong>{w.name}</strong></td>
          <td>{bytes(w.size)}</td>
          <td>{when(w.modified)}</td>
          <td>
            <button class="btn small" disabled={backingUp === w.name} onclick={() => backup(w.name)}>
              {backingUp === w.name ? '⏳ Respaldando...' : '📦 Respaldar'}
            </button>
          </td>
        </tr>
      {:else}
        <tr><td colspan="4" class="muted">Aún no hay mundos (se generan al arrancar el servidor).</td></tr>
      {/each}
    </tbody>
  </table>
</div>

