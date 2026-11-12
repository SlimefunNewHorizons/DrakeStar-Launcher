<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './components/Icon.svelte';
  import MsaModal from './components/MsaModal.svelte';
  import Toasts from './components/Toasts.svelte';
  import Welcome from './components/Welcome.svelte';
  import { t } from './lib/i18n.svelte';
  import { app, type View } from './lib/store.svelte';
  import Accounts from './views/Accounts.svelte';
  import Instances from './views/Instances.svelte';
  import Logs from './views/Logs.svelte';
  import Play from './views/Play.svelte';
  import Settings from './views/Settings.svelte';

  const nav: { id: View; icon: string }[] = [
    { id: 'play', icon: 'play' },
    { id: 'accounts', icon: 'user' },
    { id: 'instances', icon: 'box' },
    { id: 'settings', icon: 'sliders' },
    { id: 'logs', icon: 'file' },
  ];

  onMount(() => { app.init(); });
</script>

<div class="app">
  <aside class="side">
    <div class="brand"><img src="./brand/isotype.svg" alt="" /><span>DrakeStar</span></div>
    <nav class="stack" style="gap:4px" aria-label="DrakeStar">
      {#each nav as n (n.id)}
        <button type="button" class="navbtn" class:on={app.view === n.id} aria-current={app.view === n.id ? 'page' : undefined} title={t('nav.' + n.id)} onclick={() => (app.view = n.id)}>
          <Icon name={n.icon} /><span>{t('nav.' + n.id)}</span>
        </button>
      {/each}
    </nav>
    <div class="side-foot">
      v{app.info?.version ?? ''}<br />Minecraft {app.info?.min_mc ?? '1.20'} – {app.info?.max_mc ?? '26.2'}
    </div>
  </aside>

  <main>
    {#if app.update}
      <div class="banner" role="status">
        <Icon name="download" />
        <span class="grow">{t('upd.banner', { v: app.update.version })}</span>
        {#if app.busy}<span class="small muted">{t('upd.busy')}</span>{/if}
        <button class="btn sm primary" type="button" disabled={app.busy || app.updateState === 'installing'} onclick={() => app.installUpdate()}>{app.updateState === 'installing' ? t('set.installing') : t('upd.install')}</button>
      </div>
    {/if}

    {#if app.ready}
      {#if app.view === 'play'}<Play />
      {:else if app.view === 'accounts'}<Accounts />
      {:else if app.view === 'instances'}<Instances />
      {:else if app.view === 'settings'}<Settings />
      {:else}<Logs />{/if}
    {/if}
  </main>
</div>

<MsaModal />
<Welcome />
<Toasts />
