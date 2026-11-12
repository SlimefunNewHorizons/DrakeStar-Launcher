<script lang="ts">
  let { fraction = 0, text = '', showPct = true }: { fraction?: number; text?: string; showPct?: boolean } = $props();
  const pct = $derived(Math.round(Math.min(1, Math.max(0, fraction)) * 100));
</script>

<div class="progress" role="progressbar" aria-label={text} aria-valuemin="0" aria-valuemax="100" aria-valuenow={fraction > 0 ? pct : undefined}>
  <div class="bar" class:indet={fraction <= 0}><i style:width={fraction > 0 ? `${pct}%` : null}></i></div>
  <div class="ptext"><span>{text}</span>{#if fraction > 0 && showPct}<span class="mono">{pct}%</span>{/if}</div>
</div>
