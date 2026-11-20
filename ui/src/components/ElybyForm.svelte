<script lang="ts">
  import { api } from '../lib/api';
  import { errText } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  let username = $state('');
  let password = $state('');
  let totp = $state('');
  let pending = $state(false);
  let error = $state('');
  async function login() {
    if (pending) return;
    pending = true; error = '';
    try {
      await api.addElyby(username, password, totp);
      await app.loadAccounts();
      username = '';
      app.toast('ok', t('ely.success'));
    } catch (e) { error = errText(e); }
    finally { password = ''; totp = ''; pending = false; }
  }
</script>
<form class="stack" onsubmit={(e) => { e.preventDefault(); login(); }}>
  <label for="ely-username">{t('ely.username')}</label>
  <input id="ely-username" bind:value={username} autocomplete="username" maxlength="320" required disabled={pending} />
  <label for="ely-password">{t('ely.password')}</label>
  <input id="ely-password" type="password" bind:value={password} autocomplete="current-password" maxlength="1024" required disabled={pending} />
  <label for="ely-totp">{t('ely.totp')}</label>
  <input id="ely-totp" bind:value={totp} autocomplete="one-time-code" inputmode="numeric" pattern="[0-9]{6}" maxlength="6" disabled={pending} />
  {#if error}<p class="note err" role="alert">{error}</p>{/if}
  <div><button class="btn primary" type="submit" disabled={pending || app.busy}>{pending ? t('ely.working') : t('ely.login')}</button></div>
</form>
