<script lang="ts">
  import { untrack } from 'svelte';
  import { fmtRam, slug } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import type { Instance, Target } from '../lib/types';
  import Icon from './Icon.svelte';

  let { initial, onclose }: { initial: Instance | null; onclose: () => void } = $props();

  const blank = (): Instance => ({
    id: '', name: '', mc: app.versions.find((v) => v === '1.21.11') ?? app.versions[0] ?? '1.21.11', loader: 'fabric', target: { kind: 'menu' },
    performance: true, fused: false, xmx_mb: 4096, extra_jvm: [], note: '',
  });

  let draft = $state<Instance>(untrack(() => (initial ? JSON.parse(JSON.stringify(initial)) : blank())));
  let tkind = $state<Target['kind']>(untrack(() => draft.target.kind));
  let taddr = $state(untrack(() => (draft.target.kind === 'server' ? draft.target.address : '')));
  let tworld = $state(untrack(() => (draft.target.kind === 'world' ? draft.target.name : '')));
  let jvm = $state(untrack(() => draft.extra_jvm.join(' ')));
  let error = $state('');
  let saving = $state(false);
  const isNew = untrack(() => initial === null);
  const fabric = $derived(draft.loader === 'fabric');

  function setLoader(l: Instance['loader']) {
    draft.loader = l;
    if (l === 'vanilla') { draft.performance = false; draft.fused = false; }
  }

  async function save(e: Event) {
    e.preventDefault();
    const name = draft.name.trim();
    const target: Target = draft.target.kind === 'local' ? draft.target : tkind === 'server' ? { kind: 'server', address: taddr.trim() } : tkind === 'world' ? { kind: 'world', name: tworld.trim() } : { kind: 'menu' };
    const inst: Instance = {
      ...$state.snapshot(draft), id: draft.id || slug(name), name, target, mc: draft.mc.trim(),
      performance: fabric && draft.performance, fused: fabric && draft.fused, extra_jvm: jvm.split(/\s+/).filter(Boolean),
    };
    saving = true;
    error = (await app.saveInstance(inst)) ?? '';
    saving = false;
    if (!error) onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="overlay">
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="inst-title">
  <h2 id="inst-title">{isNew ? t('f.new_title') : t('f.edit_title')}</h2>
  <form class="stack" onsubmit={save}>

    <label class="field"><span>{t('f.name')}</span><input type="text" bind:value={draft.name} required maxlength="60" /></label>

    <label class="field">
      <span>{t('f.version')} <span class="muted">({t('ins.range', { min: app.info?.min_mc ?? '1.20', max: app.info?.max_mc ?? '26.2' })})</span></span>
      {#if app.versions.length}
        <select bind:value={draft.mc}>
          {#if !app.versions.includes(draft.mc)}<option value={draft.mc}>{draft.mc}</option>{/if}
          {#each app.versions as v (v)}<option value={v}>{v}</option>{/each}
        </select>
      {:else}
        <input type="text" bind:value={draft.mc} placeholder="1.21.11" required />
      {/if}
    </label>

    <div class="field"><span>{t('f.loader')}</span>
      <div class="seg">
        <button type="button" class:on={draft.loader === 'fabric'} onclick={() => setLoader('fabric')}>{t('f.fabric')}</button>
        <button type="button" class:on={draft.loader === 'vanilla'} onclick={() => setLoader('vanilla')}>{t('f.vanilla')}</button>
      </div>
    </div>

    <div class="field"><span>{t('f.target')}</span>
      <div class="seg">
        <button type="button" class:on={tkind === 'menu'} onclick={() => (tkind = 'menu')}>{t('f.t_menu')}</button>
        <button type="button" class:on={tkind === 'server'} onclick={() => (tkind = 'server')}>{t('f.t_server')}</button>
        <button type="button" class:on={tkind === 'world'} onclick={() => (tkind = 'world')}>{t('f.t_world')}</button>
      </div>
    </div>
    {#if tkind === 'server'}
      <label class="field"><span>{t('f.address')}</span><input type="text" bind:value={taddr} placeholder="mc.drakescraft.cl:25565" required spellcheck="false" /></label>
    {:else if tkind === 'world'}
      <label class="field"><span>{t('f.world')}</span><input type="text" bind:value={tworld} required /></label>
    {/if}

    <label class="switch">
      <input type="checkbox" bind:checked={draft.performance} disabled={!fabric} />
      <span class="track"></span>
      <span class="txt"><span>{t('f.perf')}</span><span class="small muted">{fabric ? t('f.perf_hint') : t('f.fabric_only')}</span></span>
    </label>
    <label class="switch">
      <input type="checkbox" bind:checked={draft.fused} disabled={!fabric} />
      <span class="track"></span>
      <span class="txt"><span>{t('f.fused')}</span><span class="small muted">{fabric ? t('f.fused_hint') : t('f.fabric_only')}</span></span>
    </label>

    <label class="field"><span>{t('f.ram')}: <strong>{fmtRam(draft.xmx_mb)}</strong></span><input type="range" min="1024" max="16384" step="512" bind:value={draft.xmx_mb} /></label>

    <details>
      <summary>{t('common.advanced')}</summary>
      <div class="stack" style="margin-top:10px">
        <label class="field"><span>{t('f.jvm')}</span><input type="text" bind:value={jvm} placeholder="-XX:+UseZGC" spellcheck="false" /></label>
        <label class="field"><span>{t('f.note')}</span><input type="text" bind:value={draft.note} maxlength="200" /></label>
      </div>
    </details>

    {#if error}<div class="note err"><Icon name="alert" /><span class="small">{error}</span></div>{/if}

    <div class="row spread foot">
      <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
      <button class="btn primary" type="submit" disabled={saving}><Icon name="check" /> {t('common.save')}</button>
    </div>
  </form>
  </div>
</div>
