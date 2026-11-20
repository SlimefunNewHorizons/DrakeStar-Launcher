<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';
  import { t } from '../lib/i18n.svelte';
  import { fmtRam } from '../lib/format';
  import type { ImportPreview } from '../lib/types';
  let { onclose }: { onclose: () => void } = $props();
  let source = $state('');
  let preview = $state<ImportPreview | null>(null);
  let checkedSource = $state('');
  let busy = $state(false);
  let error = $state('');
  async function review(e: Event) {
    e.preventDefault(); busy = true; error = ''; preview = null;
    try { checkedSource = source.trim(); preview = await api.importPreview(checkedSource); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
  async function importCopy() {
    busy = true; error = '';
    try {
      const instance = await api.importInstance(checkedSource);
      await app.loadInstances();
      app.selected = instance.id;
      onclose();
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }
</script>
<div class="overlay">
  <div class="modal stack" role="dialog" aria-modal="true" aria-labelledby="import-title">
    <h2 id="import-title">{t('import.title')}</h2>
    <p class="small muted">{t('import.hint')}</p>
    <p class="small muted">{t('import.safe')}</p>
    <form class="stack" onsubmit={review}>
      <label class="field"><span>{t('import.path')}</span><input type="text" bind:value={source} oninput={() => (preview = null)} required disabled={busy} spellcheck="false" /></label>
      <button class="btn" type="submit" disabled={busy}>{t('import.preview')}</button>
    </form>
    {#if preview}
      <div class="note"><span>{preview.instance.name} · {preview.format} · {preview.instance.mc} · {preview.instance.loader} {preview.instance.loader_version ?? ''} · {fmtRam(preview.instance.xmx_mb)}</span></div>
      <button class="btn primary" type="button" disabled={busy} onclick={importCopy}>{t('import.copy')}</button>
    {/if}
    {#if busy}<p aria-live="polite">{t('import.working')}</p>{/if}
    {#if error}<div class="note err" role="alert">{error}</div>{/if}
    <button class="btn ghost" type="button" disabled={busy} onclick={onclose}>{t('common.close')}</button>
  </div>
</div>
