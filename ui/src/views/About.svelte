<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { brand } from '../lib/brand';
  import type { JavaInfo } from '../lib/types';

  let javaPath = $state(app.settings.java_path ?? '');
  let java = $state<JavaInfo | null>(null);
  let diag = $state('');

  async function detect() {
    const p = app.active;
    if (!p) { app.notify('info', 'Crea un perfil para comprobar Java'); return; }
    const r = await app.guard(() => api.javaCheck(p.id));
    if (r) java = r.info;
  }
  async function savePath() {
    app.settings = (await app.guard(() => api.setSettings({ ...app.settings, java_path: javaPath.trim() || null }), 'Ruta de Java guardada')) ?? app.settings;
    await detect();
  }
  async function makeDiag() { diag = (await app.guard(() => api.diagnostics())) ?? ''; }
</script>

<h1>Acerca de</h1>
<div class="grid" style="grid-template-columns:repeat(auto-fit,minmax(340px,1fr))">
  <section class="card stack">
    <img src={brand.logo} alt="DrakeStar" style="max-width:260px;width:100%" />
    <dl class="kv">
      <dt>Versión</dt><dd>{app.info?.version}</dd>
      <dt>Backend</dt><dd>{#if app.info?.backend === 'tauri'}<span class="badge yes">Tauri (Rust real)</span>{:else}<span class="badge planned">MOCK de navegador</span>{/if}</dd>
      <dt>Plataforma</dt><dd>{app.info?.platform}</dd>
      <dt>Datos</dt><dd><code>{app.info?.data_dir}</code></dd>
      <dt>Cuenta</dt>
      <dd>
        {#if app.settings.session_name}
          <span class="badge yes">MSA: {app.settings.session_name}</span>
        {:else}
          <span class="badge neutral">Modo Offline (sin cuenta)</span>
        {/if}
      </dd>
    </dl>
  </section>

  <section class="card stack" aria-labelledby="a-java">
    <h2 id="a-java">Java</h2>
    <div class="field"><label for="jp">Ruta al ejecutable de Java (vacío = <code>java</code> del PATH)</label><input id="jp" bind:value={javaPath} placeholder="/usr/lib/jvm/java-25/bin/java" /></div>
    <div class="row"><button class="btn primary" onclick={savePath}>Guardar y comprobar</button><button class="btn" onclick={detect}>Comprobar</button></div>
    {#if java}
      {#if java.found}<div class="alert info">Detectado: Java {java.version} (<code>{java.bin}</code>)</div>
      {:else}<div class="alert warn">No se pudo ejecutar <code>{java.bin}</code>{java.error ? `: ${java.error}` : ''}</div>{/if}
    {/if}
  </section>

  <section class="card stack" aria-labelledby="a-lic">
    <h2 id="a-lic">Licencias</h2>
    <p>DrakeStar: MIT. Inspirado conceptualmente en <a href="https://github.com/horizon-rust/horizon" target="_blank" rel="noreferrer">Horizon</a> (MIT); este proyecto no copia su código ni su marca. Interfaz con Svelte (MIT) y Tauri (MIT/Apache-2.0).</p>
    <p class="muted">Minecraft es una marca de Mojang/Microsoft. DrakeStar no incluye ni redistribuye el juego.</p>
  </section>

  <section class="card stack" aria-labelledby="a-comm">
    <h2 id="a-comm">Comunidad y Soporte</h2>
    <p>Servidor oficial DrakesCraft Season 2, soporte técnico de la suite y asistencia de SAORI.</p>
    <div class="row">
      <a class="btn primary" href={brand.discordUrl} target="_blank" rel="noopener noreferrer" style="text-decoration:none; display:inline-flex; align-items:center; gap:0.5rem;">
        <img src={brand.icon('discord')} alt="" width="16" height="16" />
        Discord Canónico ({brand.discordUrl})
      </a>
    </div>
  </section>

  <section class="card stack" style="grid-column:1/-1" aria-labelledby="a-diag">
    <h2 id="a-diag">Diagnóstico exportable</h2>
    <p class="muted">JSON con versión, plataforma, Java (incluye la ruta del ejecutable), directorio de datos y resumen de perfiles. Cópialo manualmente para un reporte.</p>
    <button class="btn" style="align-self:start" onclick={makeDiag}>Generar</button>
    {#if diag}<label for="dg" class="sr-only">Diagnóstico</label><textarea id="dg" rows="12" readonly value={diag}></textarea>{/if}
  </section>
</div>
