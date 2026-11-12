<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';

  let send = $state(true);
  const show = $derived(app.ready && app.settings !== null && !app.settings.first_run_done);
</script>

{#if show}
  <div class="overlay">
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="wel-title">
      <img src="./brand/isotype.svg" alt="" width="56" height="56" />
      <h2 id="wel-title" style="font-size:1.35rem">{t('wel.title')}</h2>
      <p class="muted">{t('wel.lead')}</p>
      <ol class="steps">
        <li>{t('wel.s1')}</li>
        <li>{t('wel.s2')}</li>
        <li>{t('wel.s3')}</li>
      </ol>
      <label class="switch">
        <input type="checkbox" bind:checked={send} />
        <span class="track"></span>
        <span class="txt"><span>{t('wel.privacy')}</span></span>
      </label>
      <button class="btn primary" type="button" onclick={() => app.saveSettings({ first_run_done: true, send_reports: send })}>{t('wel.start')}</button>
    </div>
  </div>
{/if}
