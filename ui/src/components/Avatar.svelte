<script lang="ts">
  import type { Account } from '../lib/types';

  let { account, size = 40 }: { account: Pick<Account, 'kind' | 'name' | 'uuid'>; size?: number } = $props();
  let broken = $state(false);
  const hue = $derived([...account.name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));
  const src = $derived(account.kind === 'microsoft' ? `https://mc-heads.net/avatar/${account.uuid}/${size * 2}` : '');
  $effect(() => { src; broken = false; });
</script>

{#if src && !broken}
  <img class="avatar" {src} width={size} height={size} alt="" onerror={() => (broken = true)} />
{:else}
  <span class="avatar letter" style:width="{size}px" style:height="{size}px" style:background="hsl({hue} 55% 38%)" style:font-size="{size * 0.45}px" aria-hidden="true">{account.name.slice(0, 1).toUpperCase()}</span>
{/if}
