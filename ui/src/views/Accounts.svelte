<script lang="ts">
  import ElybyForm from '../components/ElybyForm.svelte';
  import Avatar from '../components/Avatar.svelte';
  import ConfirmButton from '../components/ConfirmButton.svelte';
  import Icon from '../components/Icon.svelte';
  import OfflineForm from '../components/OfflineForm.svelte';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
</script>

<div class="page">
  <header>
    <h1>{t('acc.title')}</h1>
    <p class="muted">{t('acc.subtitle')}</p>
  </header>

  <section class="stack">
    {#each app.accounts.accounts as a (a.id)}
      {@const isDefault = a.id === app.account?.id}
      <div class="item" class:on={isDefault}>
        <Avatar account={a} size={44} />
        <div class="grow">
          <h2 class="ellipsis">{a.name}</h2>
          <div class="row" style="gap:6px">
            <span class="chip" class:ms={a.kind === 'microsoft'}>{t('acc.' + a.kind)}</span>
            {#if isDefault}<span class="chip gold">{t('acc.default')}</span>{/if}
          </div>
        </div>
        {#if !isDefault}<button class="btn sm" type="button" onclick={() => app.useAccount(a.id)}>{t('acc.use')}</button>{/if}
        <ConfirmButton label={t('acc.remove')} onconfirm={() => app.removeAccount(a.id)} />
      </div>
    {:else}
      <p class="muted">{t('acc.empty')}</p>
    {/each}
  </section>

  <section class="grid2">
    <div class="card stack">
      <h2 class="row"><Icon name="windows" /> {t('acc.add_ms')}</h2>
      <p class="small muted">{t('acc.add_ms_hint')}</p>
      {#if app.info?.client_id_temporary}<p class="small muted">{t('acc.temp_id')}</p>{/if}
      <div><button class="btn primary" type="button" onclick={() => app.startMsa()}>{t('acc.add_ms')}</button></div>
    </div>
    <div class="card stack">
      <h2>{t('acc.add_off')}</h2>
      <p class="small muted">{t('acc.add_off_hint')}</p>
      <OfflineForm />
    </div>
    <div class="card stack">
      <h2>Ely.by</h2>
      <p class="small muted">{t('ely.hint')}</p>
      <ElybyForm />
    </div>
  </section>
</div>
