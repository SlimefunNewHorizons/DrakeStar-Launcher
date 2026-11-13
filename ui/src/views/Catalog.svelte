<script lang="ts">
  import { api } from '../lib/api';
  import { errText } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import type { CatalogEntry, CatalogHit, ContentKind } from '../lib/types';
  import ConfirmButton from '../components/ConfirmButton.svelte';
  let kind = $state<ContentKind>('mod');
  let query = $state('');
  let hits = $state<CatalogHit[]>([]);
  let installed = $state<CatalogEntry[]>([]);
  let error = $state('');
  let searching = $state(false);
  let request = 0;
  const instance = $derived(app.instance);
  const unsupported = $derived(instance?.loader !== 'fabric' && kind !== 'resourcepack');

  $effect(() => {
    const id = instance?.id;
    let active = true;
    installed = []; hits = []; error = ''; request++; searching = false;
    if (id) api.catalogList(id).then((v) => { if (active) installed = v; }).catch((e) => { if (active) error = errText(e); });
    return () => { active = false; };
  });
  async function search() {
    if (!instance || unsupported) return;
    const seq = ++request;
    searching = true; error = ''; hits = [];
    try { const found = await api.catalogSearch(instance.id, kind, query); if (seq === request) hits = found; }
    catch (e) { if (seq === request) error = errText(e); }
    finally { if (seq === request) searching = false; }
  }
  async function change(project: string, remove: boolean, contentKind = kind) {
    if (!instance || app.busy) return;
    const id = instance.id;
    app.catalogBusy = true; error = '';
    try { installed = await api.catalogChange(id, contentKind, project, remove); }
    catch (e) {
      error = errText(e);
      // Dependencias que sí se descargaron antes de un fallo también quedan visibles.
      try { installed = await api.catalogList(id); } catch { /* conservar el error original */ }
    } finally { app.catalogBusy = false; }
  }
</script>

<div class="page">
  <header><h1>{t('cat.title')}</h1><p class="muted">{t('cat.subtitle')}</p></header>
  {#if instance}
    <div class="row">
      <label for="catalog-instance">{t('nav.instances')}</label>
      <select id="catalog-instance" value={instance.id} disabled={app.busy} onchange={(e) => app.select(e.currentTarget.value)}>
        {#each app.instances as i (i.id)}<option value={i.id}>{i.name} · {i.mc} · {i.loader}</option>{/each}
      </select>
      <button class="btn" type="button" onclick={() => app.openInstanceFolder(instance.id)}>{t('common.open_folder')}</button>
    </div>
    <form class="row" onsubmit={(e) => { e.preventDefault(); search(); }}>
      <label for="catalog-kind">{t('cat.type')}</label>
      <select id="catalog-kind" bind:value={kind} disabled={app.busy} onchange={() => { request++; searching = false; hits = []; error = ''; }}>
        <option value="mod">Mods</option><option value="resourcepack">Resource packs</option><option value="shader">Shader packs</option>
      </select>
      <input aria-label={t('cat.search')} bind:value={query} maxlength="200" placeholder={t('cat.search')} />
      <button class="btn primary" type="submit" disabled={searching || app.busy || unsupported}>{searching ? t('cat.searching') : t('cat.search')}</button>
    </form>
    {#if unsupported}<p class="note warn">{t('cat.fabric')}</p>{/if}
    {#if kind === 'shader'}<p class="small muted">{t('cat.shaders')}</p>{/if}
    {#if app.catalogBusy}<p role="status">{t('cat.working')}</p>{/if}
    {#if error}<p class="note err" role="alert">{error}</p>{/if}
    <section class="stack">
      {#each hits as hit (hit.project_id)}
        <div class="item">
          <div class="grow"><h2>{hit.title}</h2><p class="small muted">{hit.description}</p></div>
          <button class="btn" type="button" disabled={app.busy || installed.some((e) => e.project_id === hit.project_id)} onclick={() => change(hit.project_id, false)}>
            {installed.some((e) => e.project_id === hit.project_id) ? t('cat.installed') : t('cat.install')}
          </button>
        </div>
      {/each}
    </section>
    <section class="stack">
      <h2>{t('cat.installed')}</h2><p class="small muted">{t('cat.managed')}</p>
      {#each installed as entry (entry.project_id)}
        <div class="item"><div class="grow"><h2>{entry.title}</h2><p class="small muted">{entry.kind} · {entry.filename}</p>{#if !entry.owned}<p class="small muted">{t('cat.external')}</p>{/if}</div>
          {#if !app.busy}<ConfirmButton label={t('cat.remove')} onconfirm={() => change(entry.project_id, true, entry.kind)} />{/if}
        </div>
      {:else}<p class="muted">{t('cat.empty')}</p>{/each}
    </section>
  {:else}<p>{t('play.no_instances')}</p>{/if}
</div>
