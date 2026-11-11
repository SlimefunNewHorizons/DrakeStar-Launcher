<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { brand, modeLabel } from '../lib/brand';
  import ChannelPill from '../components/ChannelPill.svelte';
  import ProfileEditor from './ProfileEditor.svelte';
  import { playClick, playSuccess, playError } from '../lib/sound';
  import type { Profile, PrismInstanceSummary } from '../lib/types';

  let dlg: HTMLDialogElement | undefined = $state();
  let editing = $state<{ p: Profile; creating: boolean } | null>(null);
  let importDlg: HTMLDialogElement | undefined = $state();
  let prismDlg: HTMLDialogElement | undefined = $state();
  let prismInstances = $state<PrismInstanceSummary[]>([]);
  let prismLoading = $state(false);
  let prismImportStatus = $state<string | null>(null);
  let exportDlg: HTMLDialogElement | undefined = $state();
  let importText = $state('');
  let importOverwrite = $state(false);
  let exportInfo = $state<{ id: string; json: string; path?: string } | null>(null);
  let confirmDel = $state<{ p: Profile; data: boolean } | null>(null);
  let delDlg: HTMLDialogElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();

  const blank = (): Profile => ({
    schema: 1, id: '', name: '', mode: 'drakes-java', channel: 'stable', minecraft: '26.2', loader: 'vanilla',
    java: { major: null, min_mem_mb: 1024, max_mem_mb: 4096, extra_jvm_args: [] },
    server: null, preset: null, modpack: null, notes: '',
  });

  function open(p: Profile, creating: boolean) { editing = { p, creating }; dlg?.showModal(); }
  function close() { dlg?.close(); editing = null; }
  async function saved() { close(); await app.refreshProfiles(); }

  async function duplicate(p: Profile) {
    let n = 2; const ids = new Set(app.profiles.map((x) => x.id));
    while (ids.has(`${p.id}-${n}`.slice(0, 48))) n++;
    const r = await app.guard(() => api.duplicateProfile(p.id, `${p.id}-${n}`.slice(0, 48), `${p.name} (copia)`), 'Perfil duplicado (solo la configuración, no los datos)');
    if (r) await app.refreshProfiles();
  }
  function askDelete(p: Profile) { confirmDel = { p, data: false }; delDlg?.showModal(); }
  async function doDelete() {
    if (!confirmDel) return;
    const { p, data } = confirmDel;
    const ok = await app.guard(() => api.deleteProfile(p.id, data).then(() => true), 'Perfil borrado');
    delDlg?.close(); confirmDel = null;
    if (ok) await app.refreshProfiles();
  }
  async function doExport(p: Profile) {
    const json = await app.guard(() => api.exportProfile(p.id));
    if (json === undefined) return;
    exportInfo = { id: p.id, json };
    exportDlg?.showModal();
  }
  async function exportFile() {
    if (!exportInfo) return;
    const path = await app.guard(() => api.exportProfileFile(exportInfo!.id), 'Exportado');
    if (path && exportInfo) exportInfo = { ...exportInfo, path };
  }
  async function copyJson() {
    try { await navigator.clipboard.writeText(exportInfo?.json ?? ''); app.notify('ok', 'Copiado al portapapeles'); } catch { app.notify('err', 'No se pudo copiar; selecciona el texto manualmente'); }
  }
  async function onFile(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (f) importText = await f.text();
  }
  async function doImport() {
    const r = await app.guard(() => api.importProfile(importText, importOverwrite), 'Perfil importado');
    if (r) { importDlg?.close(); importText = ''; await app.refreshProfiles(); }
  }

  async function openPrism() {
    playClick();
    prismLoading = true;
    prismImportStatus = null;
    prismDlg?.showModal();
    try {
      prismInstances = await api.prismListInstances();
    } catch (e) {
      prismImportStatus = `Error leyendo Prism: ${e}`;
    } finally {
      prismLoading = false;
    }
  }

  async function importFromPrism(inst: PrismInstanceSummary) {
    playClick();
    prismImportStatus = `Migrando «${inst.name}» (${inst.mod_count} mods)…`;
    try {
      const res = await api.prismImportInstance(inst.path);
      playSuccess();
      prismImportStatus = `¡Instancia importada! Perfil «${res.profile_name}» con ${res.mods_imported} mods listos.`;
      await app.refreshProfiles();
      app.setActive(res.profile_id);
    } catch (e) {
      playError();
      prismImportStatus = `Error al importar: ${e}`;
    }
  }
</script>

<div class="row between wrap" style="gap:1rem">
  <h1>Perfiles</h1>
  <div class="row" style="gap:.5rem">
    <button class="btn secondary" onclick={openPrism}>🔮 Importar de Prism</button>
    <button class="btn" onclick={() => { playClick(); importDlg?.showModal(); }}>Importar JSON</button>
    <button class="btn primary" onclick={() => { playClick(); open(blank(), true); }}>Nuevo perfil</button>
  </div>
</div>
<p class="muted">Cada perfil es un JSON validado por <code>drakestar-core</code> y guardado en <code>profiles/&lt;id&gt;/profile.json</code> dentro del directorio de datos{#if app.info} (<code>{app.info.data_dir}</code>){/if}.</p>

{#if app.invalid.length}
  <div class="alert warn" role="alert">
    <strong>{app.invalid.length} perfil(es) en disco no se pudieron cargar:</strong>
    <ul>{#each app.invalid as i (i.id)}<li><code>{i.id}</code>: {i.error}</li>{/each}</ul>
  </div>
{/if}

{#if app.profiles.length === 0}
  <div class="card"><p>No hay perfiles todavía. Crea uno o importa un JSON (hay ejemplos en <code>examples/</code>).</p></div>
{/if}

<div class="grid">
  {#each app.profiles as p (p.id)}
    <article class="card" class:active={app.settings.active_profile === p.id} aria-label={p.name}>
      <header>
        <img class="mi" src={brand.modeIcon(p.mode)} alt="" />
        <div class="t"><strong>{p.name}</strong><span class="muted mono">{p.id}</span></div>
        <ChannelPill channel={p.channel} />
      </header>
      <dl class="kv">
        <dt>Modo</dt><dd>{modeLabel[p.mode]}</dd>
        <dt>Minecraft</dt><dd>{p.minecraft}{p.loader !== 'vanilla' ? ` · ${p.loader}` : ''}</dd>
        <dt>Gráficos</dt>
        <dd>
          <span class="badge" class:yes={p.graphics?.backend === 'vulkan'} class:neutral={p.graphics?.backend !== 'vulkan'}>
            {p.graphics?.backend === 'vulkan' ? '⚡ Vulkan' : '🟢 OpenGL'}
          </span>
          {#if p.graphics?.width && p.graphics?.height}
            <span class="badge neutral" style="font-size:0.75rem">{p.graphics.width}×{p.graphics.height}{p.graphics.fullscreen ? ' ⛶' : ''}</span>
          {/if}
          {#if p.graphics?.discrete_gpu}
            <span class="badge yes" style="font-size:0.75rem" title="Render offload en GPU dedicada">dGPU</span>
          {/if}
        </dd>
        {#if p.server}<dt>Servidor</dt><dd>{p.server}</dd>{/if}
        {#if p.preset}<dt>Preset</dt><dd>{p.preset}</dd>{/if}
      </dl>
      {#if p.notes}<p class="muted">{p.notes}</p>{/if}
      <div class="row" style="margin-top:.75rem">
        {#if app.settings.active_profile === p.id}
          <span class="badge neutral">ACTIVO</span>
        {:else}
          <button class="btn small" onclick={() => app.setActive(p.id)}>Activar</button>
        {/if}
        <button class="btn small" onclick={() => open(p, false)}>Editar</button>
        <button class="btn small" onclick={() => duplicate(p)}>Duplicar</button>
        <button class="btn small" onclick={() => doExport(p)}>Exportar</button>
        <button class="btn small danger" onclick={() => askDelete(p)}>Borrar</button>
      </div>
    </article>
  {/each}
</div>

<dialog bind:this={dlg} aria-label="Editor de perfil" onclose={() => (editing = null)}>
  {#if editing}
    {#key editing.p.id + String(editing.creating)}
      <ProfileEditor initial={editing.p} creating={editing.creating} onsaved={saved} oncancel={close} />
    {/key}
  {/if}
</dialog>

<dialog bind:this={importDlg} aria-label="Importar perfil">
  <form method="dialog" class="stack" onsubmit={(e) => e.preventDefault()}>
    <h2>Importar perfil</h2>
    <p class="muted">Pega el JSON de un perfil o elige un archivo. Se valida con las mismas reglas que al crear (p. ej. un perfil <code>drakes-rust</code> con loader o modpack se rechaza).</p>
    <input type="file" accept="application/json,.json" bind:this={fileInput} onchange={onFile} aria-label="Archivo JSON" />
    <label for="imp-text" class="sr-only">JSON del perfil</label>
    <textarea id="imp-text" rows="10" bind:value={importText} placeholder={'{ "schema": 1, "id": "...", ... }'}></textarea>
    <label style="color:var(--text)"><input type="checkbox" bind:checked={importOverwrite} /> Sobrescribir si el id ya existe</label>
    <div class="row">
      <button class="btn primary" type="button" onclick={doImport} disabled={!importText.trim()}>Importar</button>
      <button class="btn" type="button" onclick={() => importDlg?.close()}>Cerrar</button>
    </div>
  </form>
</dialog>

<dialog bind:this={exportDlg} aria-label="Exportar perfil">
  {#if exportInfo}
    <div class="stack">
      <h2>Exportar «{exportInfo.id}»</h2>
      <label for="exp-text" class="sr-only">JSON</label>
      <textarea id="exp-text" rows="14" readonly value={exportInfo.json}></textarea>
      {#if exportInfo.path}<div class="alert info">Guardado en <code>{exportInfo.path}</code></div>{/if}
      <div class="row">
        <button class="btn" onclick={copyJson}>Copiar</button>
        <button class="btn primary" onclick={exportFile}>Guardar en la carpeta exports/</button>
        <button class="btn" onclick={() => exportDlg?.close()}>Cerrar</button>
      </div>
    </div>
  {/if}
</dialog>

<dialog bind:this={delDlg} aria-label="Confirmar borrado">
  {#if confirmDel}
    <div class="stack">
      <h2>Borrar «{confirmDel.p.name}»</h2>
      <label style="color:var(--text)"><input type="checkbox" bind:checked={confirmDel.data} /> Borrar también los datos en disco (servidor/instancia, mundos, logs)</label>
      <p class="muted">{confirmDel.data ? 'Se eliminará toda la carpeta del perfil. No se puede deshacer.' : 'Solo se borra profile.json; los datos quedan en disco.'}</p>
      <div class="row">
        <button class="btn danger" onclick={doDelete}>Borrar</button>
        <button class="btn" onclick={() => delDlg?.close()}>Cancelar</button>
      </div>
    </div>
  {/if}
</dialog>

<dialog bind:this={prismDlg} aria-label="Importar instancias desde Prism Launcher" style="max-width:650px;width:90%">
  <div class="stack">
    <div class="row between">
      <h2>🔮 Instancias Detectadas en Prism Launcher</h2>
      <button class="btn sub" onclick={() => prismDlg?.close()}>✕</button>
    </div>
    <p class="muted">
      DrakeStar detecta automáticamente tus instancias de Prism Launcher (versiones oficiales o forks).
      Al importar, se transfiere tu configuración, versión de Minecraft, Fabric y todos tus mods con verificación íntegra de SHA-256.
    </p>

    {#if prismImportStatus}
      <div class="alert info">{prismImportStatus}</div>
    {/if}

    {#if prismLoading}
      <p class="muted">Escaneando instancias en disco…</p>
    {:else if prismInstances.length === 0}
      <div class="card"><p class="muted">No se detectaron instancias de Prism Launcher en las rutas estándar.</p></div>
    {:else}
      <div class="stack" style="gap:.75rem;max-height:400px;overflow-y:auto;padding-right:.25rem">
        {#each prismInstances as inst (inst.path)}
          <div class="card row between wrap" style="align-items:center;padding:.85rem;background:rgba(255,255,255,0.03)">
            <div class="stack" style="gap:.2rem">
              <div class="row" style="gap:.5rem;align-items:center">
                <strong>{inst.name}</strong>
                <span class="badge neutral">MC {inst.minecraft_version}</span>
                {#if inst.loader_version}<span class="badge yes">Fabric {inst.loader_version}</span>{/if}
              </div>
              <span class="muted" style="font-size:0.85rem">
                📁 <code>{inst.dir_name}</code> · 📦 <strong>{inst.mod_count}</strong> mods instalados
              </span>
            </div>
            <button class="btn primary" style="font-size:0.85rem" onclick={() => importFromPrism(inst)}>
              ⬇️ Importar a DrakeStar
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="row" style="margin-top:.5rem;justify-content:flex-end">
      <button class="btn" onclick={() => prismDlg?.close()}>Cerrar</button>
    </div>
  </div>
</dialog>
