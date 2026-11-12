<script lang="ts">
  import { validOfflineName } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import Icon from './Icon.svelte';

  let { onadded }: { onadded?: () => void } = $props();
  let name = $state('');
  let touched = $state(false);
  const ok = $derived(validOfflineName(name.trim()));

  async function add(e: Event) {
    e.preventDefault();
    touched = true;
    if (!ok) return;
    if (await app.addOffline(name.trim())) {
      name = '';
      touched = false;
      onadded?.();
    }
  }
</script>

<form class="stack" onsubmit={add}>
  <label class="field">
    <span>{t('acc.name_ph')} <span class="muted">({t('acc.name_rule')})</span></span>
    <input type="text" bind:value={name} maxlength="16" autocomplete="off" spellcheck="false" placeholder="Steve" aria-invalid={touched && !ok} />
  </label>
  {#if touched && !ok}<p class="small bad">{t('acc.invalid_name')}</p>{/if}
  <div><button class="btn" type="submit"><Icon name="plus" /> {t('acc.add')}</button></div>
</form>
