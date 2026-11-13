<script lang="ts">
  import Avatar from '../components/Avatar.svelte';
  import Icon from '../components/Icon.svelte';
  import OfflineForm from '../components/OfflineForm.svelte';
  import Progress from '../components/Progress.svelte';
  import { loaderLabel, targetLabel } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';

  const inst = $derived(app.instance);
  const acc = $derived(app.account);
  const preparing = $derived(app.phase === 'preparing');
  const live = $derived(app.phase === 'running');
</script>

<div class="page">
  {#if !acc}
    <section class="card hero" style="justify-items:stretch">
      <div class="stack" style="gap:4px"><h1>{t('play.no_account_title')}</h1><p class="muted">{t('play.no_account_text')}</p></div>
      <div class="grid2">
        <div class="card stack">
          <h2 class="row"><Icon name="windows" /> {t('acc.microsoft')}</h2>
          <p class="small muted">{t('acc.add_ms_hint')}</p>
          <div><button class="btn primary" type="button" onclick={() => app.startMsa()}>{t('acc.add_ms')}</button></div>
        </div>
        <div class="card stack">
          <h2>{t('acc.add_off')}</h2>
          <p class="small muted">{t('acc.add_off_hint')}</p>
          <OfflineForm />
        </div>
      </div>
    </section>
  {:else if inst}
    <section class="card hero">
      <div class="row">
        <span class="chip gold">{loaderLabel(inst)} · {inst.mc}</span>
        <span class="muted small">{targetLabel(inst)}</span>
      </div>
      <div class="stack" style="gap:6px">
        <h1>{inst.name}</h1>
        {#if inst.note}<p class="muted">{inst.note}</p>{/if}
      </div>

      <div class="who">
        <Avatar account={acc} size={36} />
        <div>
          <label class="small muted" for="acc-select">{t('play.account')}</label>
          <div class="row" style="gap:8px">
            <select id="acc-select" value={acc.id} disabled={app.busy} onchange={(e) => app.useAccount(e.currentTarget.value)}>
              {#each app.accounts.accounts as a (a.id)}
                <option value={a.id}>{a.name} · {t('acc.' + a.kind)}</option>
              {/each}
            </select>
            <button class="btn sm ghost" type="button" onclick={() => (app.view = 'accounts')}>{t('nav.accounts')}</button>
          </div>
        </div>
      </div>

      <div class="row">
        <button class="btn ghost" type="button" onclick={() => app.openInstanceFolder(inst.id)}><Icon name="folder" /> {t('common.open_folder')}</button>
        {#if live}
          <button class="btn play live" type="button" disabled>{t('play.running')}</button>
          <button class="btn" type="button" onclick={() => app.killGame()}><Icon name="stop" /> {t('play.close_game')}</button>
        {:else if preparing}
          <button class="btn play" type="button" disabled>{t('play.preparing')}</button>
          <button class="btn ghost" type="button" onclick={() => app.cancelPlay()}>{t('common.cancel')}</button>
        {:else}
          <button class="btn play" type="button" onclick={() => app.play()}>{t('play.button')}</button>
        {/if}
      </div>

      {#if preparing}
        <Progress fraction={app.progress?.fraction ?? 0} text={app.progress?.text ?? t('play.preparing')} />
        <p class="small muted">{t('play.first_time')}</p>
      {/if}

      {#each app.warnings as w (w)}
        <div class="note warn"><Icon name="alert" /><span class="small">{w}</span></div>
      {/each}

      {#if app.phase === 'crashed' || app.phase === 'error'}
        <div class="note err">
          <Icon name="alert" />
          <div class="stack" style="gap:8px">
            <strong>{app.phase === 'crashed' ? t('play.crashed', { code: app.exitCode !== null ? ` (${app.exitCode})` : '' }) : t('play.error')}</strong>
            {#if app.message}<p class="small">{app.message}</p>{/if}
            {#if app.report}<p class="small">{app.report.sent ? t('play.report_sent') : t('play.report_saved')}</p>{/if}
            <div class="row">
              <button class="btn sm" type="button" onclick={() => (app.view = 'logs')}><Icon name="file" /> {t('play.view_logs')}</button>
              {#if !app.report?.sent}<button class="btn sm" type="button" onclick={() => app.makeReport(true)}><Icon name="send" /> {t('play.send_report')}</button>{/if}
              {#if inst.fused}<button class="btn sm" type="button" onclick={() => app.disableFused()}>{t('play.disable_fused')}</button>{/if}
            </div>
          </div>
        </div>
      {:else if app.phase === 'exited'}
        <p class="small muted">{t('play.exited')}</p>
      {:else if app.phase === 'cancelled'}
        <p class="small muted">{t('play.cancelled')}</p>
      {/if}
    </section>

    <section class="stack">
      <h2>{t('play.instances')}</h2>
      <div class="grid2">
        {#each app.instances as i (i.id)}
          <button type="button" class="card pick" class:on={i.id === inst.id} disabled={app.busy} onclick={() => app.select(i.id)}>
            <h2>{i.name}</h2>
            <span class="small muted">{loaderLabel(i)} · {i.mc} · {targetLabel(i)}</span>
          </button>
        {/each}
      </div>
    </section>
  {:else}
    <p class="muted">{t('play.no_instances')}</p>
  {/if}
</div>
