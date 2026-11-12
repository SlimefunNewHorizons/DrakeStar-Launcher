<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import Icon from './Icon.svelte';

  // Boton destructivo con confirmacion en linea (los dialogos nativos confirm() no existen en todos los webviews).
  let { label, icon = 'trash', onconfirm }: { label: string; icon?: string; onconfirm: () => void } = $props();
  let asking = $state(false);
</script>

{#if asking}
  <span class="row" style="gap:6px">
    <span class="small">{t('common.sure')}</span>
    <button class="btn sm danger" type="button" onclick={() => { asking = false; onconfirm(); }}>{t('common.yes')}</button>
    <button class="btn sm ghost" type="button" onclick={() => (asking = false)}>{t('common.no')}</button>
  </span>
{:else}
  <button class="btn sm danger" type="button" onclick={() => (asking = true)}><Icon name={icon} /> {label}</button>
{/if}
