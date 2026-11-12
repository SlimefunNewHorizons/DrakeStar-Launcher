<script lang="ts">
  import { api } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';
  import Progress from './Progress.svelte';

  let left = $state(0);
  let copied = $state(false);

  // cuenta regresiva del codigo mientras se espera al navegador
  $effect(() => {
    const code = app.msa.code;
    if (app.msa.step !== 'waiting' || !code) return;
    left = code.expires_in;
    const id = setInterval(() => (left = Math.max(0, left - 1)), 1000);
    return () => clearInterval(id);
  });

  const url = $derived(app.msa.code ? (app.msa.code.verification_uri_complete ?? app.msa.code.verification_uri) : '');
  const mm = $derived(String(Math.floor(left / 60)));
  const ss = $derived(String(left % 60).padStart(2, '0'));

  async function copy() {
    try {
      await navigator.clipboard.writeText(app.msa.code?.user_code ?? '');
      copied = true;
      setTimeout(() => (copied = false), 1800);
    } catch { /* el codigo se puede seleccionar con un clic */ }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && app.msa.open && app.cancelMsa()} />

{#if app.msa.open}
  <div class="overlay">
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="msa-title">
      <h2 id="msa-title" class="row"><Icon name="windows" /> {t('msa.title')}</h2>

      {#if app.msa.step === 'starting'}
        <Progress text={t('msa.opening')} />
      {:else if app.msa.step === 'waiting' && app.msa.code}
        <p>{t('msa.waiting')}</p>
        <code class="code" aria-label={app.msa.code.user_code}>{app.msa.code.user_code}</code>
        <div class="row">
          <button class="btn sm" type="button" onclick={copy}><Icon name={copied ? 'check' : 'copy'} /> {copied ? t('common.copied') : t('common.copy')}</button>
          <button class="btn sm ghost" type="button" onclick={() => api.openUrl(url)}><Icon name="link" /> {t('msa.reopen')}</button>
        </div>
        <Progress fraction={left / (app.msa.code.expires_in || 1)} showPct={false} text={t('msa.expires', { min: mm, sec: ss })} />
      {:else if app.msa.step === 'working'}
        <Progress text={app.msa.message || t('msa.working')} />
      {:else}
        <div class="note err"><Icon name="alert" /><div><strong>{t('msa.failed')}</strong><p class="small">{app.msa.message}</p></div></div>
      {/if}

      <div class="row spread">
        {#if app.msa.step === 'error'}
          <button class="btn primary" type="button" onclick={() => app.startMsa()}><Icon name="refresh" /> {t('common.retry')}</button>
        {:else}<span></span>{/if}
        <button class="btn ghost" type="button" onclick={() => app.cancelMsa()}>{app.msa.step === 'error' ? t('common.close') : t('common.cancel')}</button>
      </div>
    </div>
  </div>
{/if}
