<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { sound } from '../lib/sound';
  import type { DeviceCodeAuth } from '../lib/types';

  let dlg: HTMLDialogElement | undefined = $state();
  let loading = $state(false);
  let authInfo = $state<DeviceCodeAuth | null>(null);
  let pollTimer = $state<any>(null);
  let statusText = $state<string>('');
  let copied = $state(false);

  export function openModal() {
    sound.click();
    authInfo = null;
    statusText = '';
    copied = false;
    dlg?.showModal();
    if (!app.settings.session_token) {
      startLogin();
    }
  }

  export function closeModal() {
    stopPolling();
    dlg?.close();
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function startLogin() {
    loading = true;
    statusText = 'Solicitando código de autorización a Microsoft...';
    try {
      const res = await api.msftLoginStart();
      authInfo = res;
      loading = false;
      statusText = `Esperando autorización en ${res.verification_uri}...`;
      beginPolling(res.device_code, res.interval);
    } catch (e: any) {
      loading = false;
      statusText = `Error al conectar con Microsoft: ${e?.message || e}`;
      sound.error();
    }
  }

  function beginPolling(deviceCode: string, intervalSeconds: number) {
    stopPolling();
    const sec = Math.max(intervalSeconds, 5);
    pollTimer = setInterval(async () => {
      try {
        const res = await api.msftLoginPoll(deviceCode);
        if (res.status === 'Success') {
          stopPolling();
          app.settings.session_uuid = res.payload.uuid;
          app.settings.session_token = res.payload.access_token;
          app.settings.session_name = res.payload.name;
          statusText = `¡Sesión iniciada con éxito como ${res.payload.name}!`;
          sound.success();
          app.notify('ok', `🎮 Cuenta de Minecraft vinculada: ${res.payload.name}`);
        } else if (res.status === 'Pending') {
          statusText = `Esperando que ingreses el código en el navegador (${res.payload}s)...`;
        }
      } catch (e: any) {
        stopPolling();
        statusText = `Autorización finalizada o rechazada: ${e?.message || e}`;
        sound.error();
      }
    }, sec * 1000);
  }

  async function logout() {
    sound.click();
    stopPolling();
    try {
      const st = await api.msftLogout();
      app.settings = st;
      authInfo = null;
      statusText = 'Sesión cerrada correctamente.';
      app.notify('info', 'Sesión de Microsoft desvinculada.');
    } catch (e: any) {
      app.notify('err', `Error al cerrar sesión: ${e?.message || e}`);
    }
  }

  async function copyCode() {
    if (!authInfo) return;
    try {
      await navigator.clipboard.writeText(authInfo.user_code);
      copied = true;
      sound.click();
      setTimeout(() => (copied = false), 2500);
    } catch {
      app.notify('err', 'No se pudo copiar el código');
    }
  }
</script>

<dialog bind:this={dlg} aria-label="Cuenta Microsoft Minecraft" class="msft-dialog" onclose={stopPolling}>
  <div class="stack" style="gap:1rem;max-width:480px;width:100%">
    <div class="row between items-center">
      <div class="row items-center" style="gap:.5rem">
        <span style="font-size:1.5rem">🟩</span>
        <h2 style="margin:0">Cuenta Microsoft · Minecraft</h2>
      </div>
      <button class="btn sub" type="button" onclick={closeModal} aria-label="Cerrar modal">✕</button>
    </div>

    {#if app.settings.session_token && app.settings.session_name}
      <div class="card row items-center" style="gap:1rem;padding:1rem;background:rgba(46, 204, 113, 0.08);border:1px solid rgba(46,204,113,0.3)">
        <div class="avatar-placeholder" style="width:48px;height:48px;border-radius:4px;background:#1b2430;display:flex;align-items:center;justify-content:center;font-size:1.5rem">
          👤
        </div>
        <div class="stack" style="gap:.2rem;flex:1">
          <div class="row items-center" style="gap:.5rem">
            <strong>{app.settings.session_name}</strong>
            <span class="badge yes">ONLINE-MODE</span>
          </div>
          <span class="muted mono" style="font-size:0.75rem">{app.settings.session_uuid}</span>
        </div>
      </div>
      <p class="muted" style="font-size:0.85rem">
        Tu cliente usará esta identidad verificada para unirse a servidores oficiales como <code>mc.drakescraft.cl</code> con cifrado RSA + AES-128-CFB8 y handshake Mojang.
      </p>
      <div class="row between" style="margin-top:.5rem">
        <button class="btn danger" type="button" onclick={logout}>Desvincular cuenta</button>
        <button class="btn primary" type="button" onclick={closeModal}>Listo</button>
      </div>
    {:else}
      <p class="muted" style="font-size:0.9rem">
        Inicia sesión con tu cuenta Microsoft para jugar en servidores online-mode. No necesitas contraseña en la app: autorizas de forma segura mediante <strong>OAuth 2.0 Device Code</strong> en el navegador oficial de Microsoft.
      </p>

      {#if loading}
        <div class="alert info">Conectando con Microsoft Authentication Services…</div>
      {:else if authInfo}
        <div class="auth-box stack" style="gap:.75rem;padding:1rem;background:rgba(255,255,255,0.03);border:1px solid rgba(255,255,255,0.1);border-radius:6px;text-align:center">
          <span class="muted" style="font-size:0.85rem">1. Abre este enlace oficial de Microsoft:</span>
          <a
            class="tag-link"
            href={authInfo.verification_uri}
            target="_blank"
            rel="noopener noreferrer"
            style="font-size:1rem;padding:.4rem .8rem;text-decoration:underline"
          >
            🔗 {authInfo.verification_uri}
          </a>

          <span class="muted" style="font-size:0.85rem;margin-top:.5rem">2. Ingresa este código temporal:</span>
          <div class="row center" style="gap:.5rem;justify-content:center">
            <span class="mono" style="font-size:1.6rem;font-weight:700;letter-spacing:2px;background:rgba(0,0,0,0.4);padding:.3rem .8rem;border-radius:4px;color:var(--accent,#38ef7d)">
              {authInfo.user_code}
            </span>
            <button class="btn small" type="button" onclick={copyCode}>
              {copied ? '✓ ¡Copiado!' : 'Copiar'}
            </button>
          </div>
        </div>

        {#if statusText}
          <div class="alert info" style="font-size:0.85rem">{statusText}</div>
        {/if}
      {:else}
        {#if statusText}
          <div class="alert warn" style="font-size:0.85rem">{statusText}</div>
        {/if}
        <button class="btn primary" type="button" onclick={startLogin}>Iniciar sesión con Microsoft</button>
      {/if}

      <div class="row" style="justify-content:flex-end;margin-top:.5rem">
        <button class="btn" type="button" onclick={closeModal}>Cancelar</button>
      </div>
    {/if}
  </div>
</dialog>

<style>
  .msft-dialog::backdrop {
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(4px);
  }
</style>
