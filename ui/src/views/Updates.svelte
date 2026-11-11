<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { bytes } from '../lib/format';
  import ChannelPill from '../components/ChannelPill.svelte';
  import type { Resolution } from '../lib/types';

  let count = $state<number | null>(null);
  let text = $state('');
  let rows = $state<Resolution[]>([]);

  async function load() {
    count = (await app.guard(() => api.manifestSummary())) ?? null;
    const out: Resolution[] = [];
    for (const p of app.profiles) { const r = await app.guard(() => api.resolveRelease(p.id)); if (r) out.push(r); }
    rows = out;
  }
  $effect(() => { void app.profiles.length; void load(); });
  async function onFile(e: Event) { const f = (e.currentTarget as HTMLInputElement).files?.[0]; if (f) text = await f.text(); }
  async function imp() {
    const n = await app.guard(() => api.manifestImport(text), 'Manifiesto importado');
    if (n !== undefined) { text = ''; await load(); }
  }
  const nameOf = (id: string) => app.profiles.find((p) => p.id === id)?.name ?? id;
</script>

<h1>Actualizaciones</h1>
<div class="alert info">Manifiesto integrado oficial (<code>manifest.json</code>) con releases firmadas por SHA-256 para <strong>DrakesPurpur 26.2.140</strong> y <strong>DrakesRust Client</strong>. La resolución utiliza <code>Manifest::resolve</code> y respeta el canal (stable / unstable / nightly) y la versión de Minecraft de cada perfil.</div>

<div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(340px,1fr))">
  <section class="card stack" aria-labelledby="u-imp">
    <h2 id="u-imp">Manifiesto de releases</h2>
    <p>{count === null ? 'No hay manifiesto importado.' : `Manifiesto importado: ${count} release(s).`}</p>
    <input type="file" accept="application/json,.json" onchange={onFile} aria-label="Archivo de manifiesto" />
    <label for="mf" class="sr-only">JSON del manifiesto</label>
    <textarea id="mf" rows="8" bind:value={text} placeholder={'{ "schema": 1, "releases": [ ... ] }'}></textarea>
    <button class="btn primary" style="align-self:start" onclick={imp} disabled={!text.trim()}>Importar manifiesto</button>
  </section>

  <section class="card stack" style="grid-column:1/-1" aria-labelledby="u-res">
    <h2 id="u-res">Resolución por perfil</h2>
    <div class="tablewrap">
      <table>
        <thead><tr><th scope="col">Perfil</th><th scope="col">Canal</th><th scope="col">Componente</th><th scope="col">Release resuelta</th><th scope="col">SHA-256</th></tr></thead>
        <tbody>
          {#each rows as r (r.profile)}
            <tr>
              <td>{nameOf(r.profile)}<div class="muted mono">MC {r.minecraft}</div></td>
              <td><ChannelPill channel={r.profile_channel} /></td>
              <td>{r.component ?? '—'}</td>
              <td>{#if r.resolved}<strong>{r.resolved.version}</strong> <ChannelPill channel={r.resolved.release_channel} /><div class="muted">{r.resolved.size ? bytes(r.resolved.size) : 'tamaño desconocido'}</div>{:else}<span class="muted">{r.reason}</span>{/if}</td>
              <td>{#if r.resolved}{#if r.resolved.sha256_wellformed}<code>{r.resolved.sha256.slice(0, 16)}…</code> <span class="badge neutral">formato válido (no verificado hasta descargar)</span>{:else}<span class="badge no">malformado</span>{/if}{:else}—{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
</div>
