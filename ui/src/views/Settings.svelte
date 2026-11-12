<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '../lib/api';
  import Icon from '../components/Icon.svelte';
  import { currentLang } from '../lib/i18n.svelte';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';

  const s = $derived(app.settings);
  let clientId = $state(untrack(() => app.settings?.azure_client_id ?? ''));
  let endpoint = $state(untrack(() => app.settings?.report_endpoint ?? ''));
  const checking = $derived(app.updateState === 'checking');
  const installing = $derived(app.updateState === 'installing');
</script>

<div class="page">
  <header><h1>{t('set.title')}</h1></header>

  {#if s}
    <section class="card stack">
      <div class="row spread">
        <h2>{t('set.updates')}</h2>
        <span class="muted small">{t('set.version', { v: app.info?.version ?? '' })}</span>
      </div>
      {#if app.update}
        <div class="note warn"><Icon name="download" /><span>{t('set.update_avail', { v: app.update.version })}</span></div>
        <div class="row">
          <button class="btn primary" type="button" disabled={app.busy || installing} onclick={() => app.installUpdate()}><Icon name="download" /> {installing ? t('set.installing') : t('set.install')}</button>
          {#if app.busy}<span class="small muted">{t('set.close_game_first')}</span>{/if}
        </div>
      {:else}
        <div><button class="btn" type="button" disabled={checking} onclick={() => app.checkUpdates(true)}><Icon name="refresh" /> {checking ? t('set.checking') : t('set.check')}</button></div>
      {/if}
      <label class="switch">
        <input type="checkbox" checked={s.auto_update} onchange={(e) => app.saveSettings({ auto_update: e.currentTarget.checked })} />
        <span class="track"></span>
        <span class="txt"><span>{t('set.auto')}</span></span>
      </label>
    </section>

    <section class="card stack">
      <h2>{t('set.privacy')}</h2>
      <label class="switch">
        <input type="checkbox" checked={s.send_reports} onchange={(e) => app.saveSettings({ send_reports: e.currentTarget.checked })} />
        <span class="track"></span>
        <span class="txt"><span>{t('set.reports')}</span><span class="small muted">{t('set.reports_hint')}</span></span>
      </label>
    </section>

    <section class="card stack">
      <label class="field">
        <span>{t('set.language')}</span>
        <select value={currentLang()} onchange={(e) => app.saveSettings({ language: e.currentTarget.value })}>
          <option value="es">Español</option>
          <option value="en">English</option>
        </select>
      </label>
      <div class="row">
        <button class="btn" type="button" onclick={() => api.openFolder('data')}><Icon name="folder" /> {t('set.open_data')}</button>
        <button class="btn" type="button" onclick={() => api.openFolder('logs')}><Icon name="folder" /> {t('set.open_logs')}</button>
      </div>
    </section>

    <details class="card">
      <summary>{t('common.advanced')}</summary>
      <div class="stack" style="margin-top:12px">
        <label class="field">
          <span>{t('set.client_id')} <span class="muted">— {t('set.client_id_hint')}</span></span>
          <input type="text" bind:value={clientId} spellcheck="false" placeholder="00000000-0000-0000-0000-000000000000" onchange={() => app.saveSettings({ azure_client_id: clientId.trim() || null }, false)} />
        </label>
        <label class="field">
          <span>{t('set.endpoint')}</span>
          <input type="text" bind:value={endpoint} spellcheck="false" placeholder="https://…" onchange={() => app.saveSettings({ report_endpoint: endpoint.trim() || null }, false)} />
        </label>
        <p class="small muted">{t('set.device')}: <span class="mono">{s.device_id}</span></p>
        <p class="small muted">{t('set.supported')}: <span class="mono">{app.info?.min_mc} – {app.info?.max_mc}</span> · <span class="mono">{app.info?.os} {app.info?.arch}</span></p>
        <p class="small muted mono ellipsis">{app.info?.data_dir}</p>
      </div>
    </details>
  {/if}
</div>
