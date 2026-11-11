<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import ProfileSummary from '../components/ProfileSummary.svelte';
  import CompatTable from '../components/CompatTable.svelte';
  import type { Profile, McStatus } from '../lib/types';

  let { p }: { p: Profile } = $props();
  let result = $state<Record<string, { ms?: number; status?: McStatus; err?: string; busy?: boolean }>>({});
  const online = $derived(app.profiles.filter((x) => x.mode === 'online'));

  function cleanMotd(raw: string | null | undefined): string {
    if (!raw) return '';
    return raw.replace(/\\n/g, ' ').replace(/§[0-9a-fk-or]/gi, '').trim();
  }

  async function pingSlp(q: Profile) {
    if (!q.server) return;
    result[q.id] = { busy: true };
    try {
      const st = await api.pingMcStatus(q.server);
      result[q.id] = { ms: st.latency_ms, status: st };
    } catch {
      // Fallback automatico a Ping TCP si el servidor no soporta SLP o esta arrancando
      try {
        const ms = await api.ping(q.server);
        result[q.id] = { ms };
      } catch (e) {
        result[q.id] = { err: e instanceof Error ? e.message : String(e) };
      }
    }
  }

  async function pingTcp(q: Profile) {
    if (!q.server) return;
    result[q.id] = { busy: true };
    try {
      const ms = await api.ping(q.server);
      result[q.id] = { ms };
    } catch (e) {
      result[q.id] = { err: e instanceof Error ? e.message : String(e) };
    }
  }
</script>

<h1>Online — {p.name}</h1>
<div class="alert info">
  La comprobación <strong>SLP (Server List Ping)</strong> consulta el protocolo nativo de Minecraft para obtener latencia precisa, jugadores conectados, versión y MOTD. Si el servidor no soporta SLP, conmuta automáticamente a <strong>Ping TCP</strong>.
</div>

<div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(340px,1fr))">
  <section class="card stack">
    <h2>Perfil activo</h2>
    <ProfileSummary {p} />
  </section>

  <section class="card stack">
    <h2>Accesos rápidos</h2>
    {#if online.length === 0}
      <div class="hint">No hay perfiles configurados en modo online.</div>
    {/if}
    {#each online as q (q.id)}
      <div class="online-server-item card stack" style="margin-bottom: 0.75rem; background: rgba(0,0,0,0.25);">
        <div class="row between wrap">
          <div class="row" style="gap: 0.6rem;">
            {#if result[q.id]?.status?.favicon}
              <img src={result[q.id].status!.favicon!} alt="Icon" width="28" height="28" style="border-radius: 4px; image-rendering: pixelated;" />
            {/if}
            <div>
              <strong>{q.name}</strong>
              <div class="mono muted" style="font-size: 0.8rem;">{q.server}</div>
            </div>
          </div>
          <div class="row" style="gap: 0.4rem;">
            {#if result[q.id]?.busy}
              <span class="muted" style="font-size: 0.85rem;">comprobando…</span>
            {:else if result[q.id]?.status}
              <span class="badge yes" title="SLP exitoso">⚡ {result[q.id].status!.latency_ms} ms</span>
              {#if result[q.id].status!.players_online !== null}
                <span class="badge info" title="Jugadores en línea">👥 {result[q.id].status!.players_online} / {result[q.id].status!.players_max ?? '?'}</span>
              {/if}
            {:else if result[q.id]?.ms !== undefined}
              <span class="badge yes" title="TCP ping">⚡ {result[q.id].ms} ms (TCP)</span>
            {:else if result[q.id]?.err}
              <span class="badge no" title={result[q.id].err}>sin respuesta</span>
            {/if}
            <button class="btn small" onclick={() => pingSlp(q)} disabled={result[q.id]?.busy}>Estado SLP</button>
            <button class="btn small" onclick={() => pingTcp(q)} disabled={result[q.id]?.busy}>TCP</button>
          </div>
        </div>

        {#if result[q.id]?.status}
          <div class="server-details row between wrap" style="font-size: 0.75rem; color: #9ca3af; border-top: 1px dashed rgba(255,255,255,0.08); padding-top: 0.4rem;">
            {#if result[q.id].status!.version_name}
              <span>Versión: <strong style="color: #e5e7eb;">{result[q.id].status!.version_name}</strong></span>
            {/if}
            {#if result[q.id].status!.description}
              <span class="motd-preview mono" style="color: #d1d5db; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                {cleanMotd(result[q.id].status!.description)}
              </span>
            {/if}
          </div>
        {/if}

        {#if result[q.id]?.err}
          <div class="hint text-danger" style="color: #ef4444; font-size: 0.8rem;">{result[q.id].err}</div>
        {/if}
      </div>
    {/each}
  </section>

  <section class="card stack">
    <h2>Compatibilidades</h2>
    <CompatTable mode="online" />
  </section>
</div>
