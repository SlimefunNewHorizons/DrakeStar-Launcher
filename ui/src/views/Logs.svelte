<script lang="ts">
  import { tick } from 'svelte';
  import { api } from '../lib/api';
  import Icon from '../components/Icon.svelte';
  import { errText, lineClass } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';

  const failed = $derived(app.phase === 'error' || app.phase === 'crashed');
  const instance = $derived(app.attemptInstance ?? app.instance);
  let loadError = $state('');
  let tab = $state<'launcher' | 'game'>('launcher');
  let saved = $state<string[]>([]);
  let loadRequest = 0;
  let box = $state<HTMLDivElement>();

  // En la pestana del juego, mientras hay una partida de esta sesion se muestra su salida en vivo; si no, el ultimo latest.log.
  const live = $derived(tab === 'game' && app.gameLines.length > 0);
  const shown = $derived(live ? app.gameLines : saved);

  async function load() {
    const request = ++loadRequest;
    loadError = '';
    saved = [];
    try {
      const lines = await api.logs(tab, tab === 'game' ? (instance?.id ?? null) : null, 400);
      if (request === loadRequest) saved = lines;
    } catch (e) {
      if (request === loadRequest) loadError = errText(e);
    }
  }
  $effect(() => { tab; instance?.id; load(); });
  $effect(() => {
    shown.length;
    tick().then(() => box && (box.scrollTop = box.scrollHeight));
  });
</script>

<div class="page">
  <header>
    <h1>{t('log.title')}</h1>
    <p class="muted">{t('log.subtitle')}</p>
  </header>

  {#if failed}
    <section class="card stack" role="alert">
      <h2>{app.phase === 'crashed' ? t('play.crashed', { code: app.exitCode !== null ? ` (${app.exitCode})` : '' }) : t('play.error')}</h2>
      <p>{instance?.name} · {instance?.mc}</p>
      {#if app.failedStage}<p class="small muted">{app.failedStage}</p>{/if}
      {#if app.message}<pre style="white-space:pre-wrap;overflow-wrap:anywhere">{app.message}</pre>{/if}
      <p class="small muted">{t('diag.recovery')}</p>
      <div class="row">
        <button class="btn primary" type="button" disabled={app.busy} onclick={() => { if (instance) app.select(instance.id); app.view = 'play'; }}>{t('diag.back')}</button>
        <button class="btn" type="button" onclick={() => (app.view = 'instances')}>{t('ins.edit')}</button>
        <button class="btn" type="button" onclick={() => (app.view = 'accounts')}>{t('nav.accounts')}</button>
        {#if instance}<button class="btn" type="button" onclick={() => app.openInstanceFolder(instance.id)}><Icon name="folder" /> {t('common.open_folder')}</button>{/if}
      </div>
    </section>
  {/if}
  {#if loadError}<p class="note err" role="alert">{loadError}</p>{/if}

  <div class="row spread">
    <div class="seg" style="max-width:320px">
      <button type="button" class:on={tab === 'launcher'} onclick={() => (tab = 'launcher')}>{t('log.launcher')}</button>
      <button type="button" class:on={tab === 'game'} onclick={() => (tab = 'game')}>{t('log.game')}</button>
    </div>
    <div class="row">
      <button class="btn sm" type="button" onclick={load}><Icon name="refresh" /> {t('log.refresh')}</button>
      <button class="btn sm ghost" type="button" onclick={() => api.openFolder('logs')}><Icon name="folder" /> {t('common.open_folder')}</button>
    </div>
  </div>

  {#if tab === 'game'}<p class="small muted">{live ? t('log.live') : t('log.file')}</p>{/if}

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="logbox" bind:this={box} tabindex="0" role="log" aria-live="off">
    {#each shown as line, i (i)}<div class="ln {lineClass(line)}">{line}</div>{:else}<span class="muted">{t('log.empty')}</span>{/each}
  </div>

  <div class="row">
    <button class="btn" type="button" onclick={() => app.makeReport(false)}><Icon name="file" /> {t('log.create')}</button>
    <button class="btn primary" type="button" onclick={() => app.makeReport(true)}><Icon name="send" /> {t('log.send')}</button>
  </div>
</div>
