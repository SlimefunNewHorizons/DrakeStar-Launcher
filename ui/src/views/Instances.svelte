<script lang="ts">
  import { api } from '../lib/api';
  import ConfirmButton from '../components/ConfirmButton.svelte';
  import Icon from '../components/Icon.svelte';
  import ImportForm from '../components/ImportForm.svelte';
  import InstanceForm from '../components/InstanceForm.svelte';
  import { fmtRam, loaderLabel, targetLabel } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import type { Instance } from '../lib/types';

  let importing = $state(false);
  // undefined = formulario cerrado, null = instancia nueva
  let editing = $state<Instance | null | undefined>(undefined);
</script>

<div class="page">
  <header>
    <div class="row spread" style="align-items:flex-start">
      <div style="flex:1 1 260px">
        <h1>{t('ins.title')}</h1>
        <p class="muted">{t('ins.subtitle')}</p>
      </div>
      <button class="btn" type="button" onclick={() => (importing = true)}>{t('import.title')}</button>
      <button class="btn primary" type="button" onclick={() => (editing = null)}><Icon name="plus" /> {t('ins.new')}</button>
    </div>
  </header>

  <section class="stack">
    {#each app.instances as i (i.id)}
      <div class="item" class:on={i.id === app.instance?.id}>
        <div class="stack" style="gap:6px; flex:1 1 240px; min-width:0">
          <h2 class="ellipsis">{i.name}</h2>
          <div class="row small muted" style="gap:8px">
            <span class="chip gold">{loaderLabel(i)} · {i.mc}</span>
            <span>{targetLabel(i)}</span>
            <span>{fmtRam(i.xmx_mb)}</span>
            {#if i.performance}<span class="chip">{t('f.perf')}</span>{/if}
            {#if i.fused}<span class="chip">Fused</span>{/if}
          </div>
        </div>
        <div class="row" style="gap:8px; flex:none">
          <button class="btn sm" type="button" onclick={() => (editing = i)}><Icon name="edit" /> {t('ins.edit')}</button>
          <button class="btn sm ghost" type="button" title={t('common.open_folder')} aria-label={t('common.open_folder')} onclick={() => api.openFolder('instance', i.id)}><Icon name="folder" /></button>
          <ConfirmButton label={t('ins.delete')} onconfirm={() => app.deleteInstance(i.id)} />
        </div>
      </div>
    {/each}
  </section>
</div>

{#if editing !== undefined}
  {#key editing?.id ?? 'new'}
    <InstanceForm initial={editing} onclose={() => (editing = undefined)} />
  {/key}
{/if}

{#if importing}<ImportForm onclose={() => (importing = false)} />{/if}
