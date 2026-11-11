<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/state.svelte';
  import { brand, modeLabel } from '../lib/brand';
  import { slug } from '../lib/format';
  import type { Channel, Loader, Mode, Profile, GraphicsBackend } from '../lib/types';
  import CompatTable from '../components/CompatTable.svelte';

  let { initial, creating, onsaved, oncancel }: {
    initial: Profile; creating: boolean; onsaved: (p: Profile) => void; oncancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let p = $state<Profile>(structuredClone($state.snapshot(initial)));
  // svelte-ignore state_referenced_locally
  let jvmArgs = $state(initial.java.extra_jvm_args.join(' '));
  // svelte-ignore state_referenced_locally
  let idTouched = $state(!creating);
  // svelte-ignore state_referenced_locally
  let graphicsBackend = $state<GraphicsBackend>(initial.graphics?.backend ?? 'opengl');
  // svelte-ignore state_referenced_locally
  let graphicsWidth = $state<number | null>(initial.graphics?.width ?? 1280);
  // svelte-ignore state_referenced_locally
  let graphicsHeight = $state<number | null>(initial.graphics?.height ?? 720);
  // svelte-ignore state_referenced_locally
  let graphicsFullscreen = $state<boolean>(initial.graphics?.fullscreen ?? false);
  // svelte-ignore state_referenced_locally
  let graphicsDiscreteGpu = $state<boolean>(initial.graphics?.discrete_gpu ?? true);
  let serverError = $state<string | null>(null);
  let saving = $state(false);

  const modes: Mode[] = ['drakes-rust', 'drakes-java', 'local-server', 'online'];
  const channels: Channel[] = ['stable', 'unstable', 'nightly'];
  const loaders: Loader[] = ['vanilla', 'fabric', 'forge', 'neoforge'];
  const compat = $derived(app.compatOf(p.mode));

  // Al cambiar de modo se limpian los campos que ese modo no admite (la validacion final es la de drakestar-core).
  function setMode(m: Mode) {
    p.mode = m;
    const c = app.compatOf(m);
    if (c && !c.allowed_loaders.includes(p.loader)) p.loader = 'vanilla';
    if (!c?.modpacks_allowed) p.modpack = null;
    if (m !== 'local-server') p.preset = null;
    if (m === 'local-server' && !p.preset) p.preset = 'slimefun';
    if (m !== 'online' && m !== 'drakes-rust' && m !== 'drakes-java') p.server = null;
    if (m === 'online' && !p.server) p.server = '';
  }
  function onName() {
    if (creating && !idTouched) p.id = slug(p.name);
  }
  function snapshot(): Profile {
    const q = structuredClone($state.snapshot(p)) as Profile;
    q.java.extra_jvm_args = jvmArgs.trim() ? jvmArgs.trim().split(/\s+/) : [];
    q.java.major = q.java.major || null;
    q.server = q.server?.trim() ? q.server.trim() : null;
    q.modpack = q.modpack?.trim() ? q.modpack.trim() : null;
    q.preset = q.preset?.trim() ? q.preset.trim() : null;
    q.graphics = {
      backend: graphicsBackend,
      width: graphicsWidth ? Number(graphicsWidth) : 1280,
      height: graphicsHeight ? Number(graphicsHeight) : 720,
      fullscreen: graphicsFullscreen,
      discrete_gpu: graphicsDiscreteGpu,
    };
    return q;
  }
  // Validacion en vivo con drakestar-core (comando validate_profile).
  $effect(() => {
    const q = snapshot();
    let cancelled = false;
    const t = setTimeout(async () => {
      try { const e = await api.validateProfile(q); if (!cancelled) serverError = e; } catch (e) { if (!cancelled) serverError = String(e); }
    }, 150);
    return () => { cancelled = true; clearTimeout(t); };
  });

  async function save() {
    saving = true;
    const q = snapshot();
    const r = await app.guard(() => (creating ? api.createProfile(q) : api.updateProfile(q)), creating ? 'Perfil creado' : 'Perfil guardado');
    saving = false;
    if (r) onsaved(r);
  }
</script>

<form onsubmit={(e) => { e.preventDefault(); void save(); }} class="stack">
  <h2>{creating ? 'Nuevo perfil' : `Editar «${initial.name}»`}</h2>

  <fieldset class="field" style="border:0;padding:0;margin:0">
    <legend class="muted" style="font-size:.85rem">Modo</legend>
    <div class="choice" role="radiogroup" aria-label="Modo">
      {#each modes as m (m)}
        <label>
          <input type="radio" name="mode" value={m} checked={p.mode === m} onchange={() => setMode(m)} />
          <img src={brand.modeIcon(m)} alt="" /> {modeLabel[m]}
        </label>
      {/each}
    </div>
  </fieldset>

  <div class="form-grid">
    <div class="field">
      <label for="pf-name">Nombre</label>
      <input id="pf-name" bind:value={p.name} oninput={onName} required />
    </div>
    <div class="field">
      <label for="pf-id">Id (a-z, 0-9, - y _)</label>
      <input id="pf-id" bind:value={p.id} oninput={() => (idTouched = true)} disabled={!creating} required pattern="[a-z0-9_\-]{'{1,48}'}" />
      {#if !creating}<div class="hint">El id no se puede cambiar; duplica el perfil para uno nuevo.</div>{/if}
    </div>
    <div class="field">
      <label for="pf-mc">Versión de Minecraft</label>
      <input id="pf-mc" bind:value={p.minecraft} list="mc-versions" required placeholder="p. ej. 26.2 o 1.21.4" />
      <datalist id="mc-versions">
        <option value="26.2">26.2 (DrakesPurpur / DrakesRust)</option>
        <option value="1.21.4">1.21.4 (LTS reciente)</option>
        <option value="1.21.1">1.21.1 (Producción Dallas)</option>
        <option value="1.20.4">1.20.4</option>
        <option value="1.20.1">1.20.1 (Ecosistema Mods)</option>
        <option value="1.19.4">1.19.4</option>
        <option value="1.16.5">1.16.5 (Legacy)</option>
      </datalist>
    </div>
  </div>

  <fieldset class="field" style="border:0;padding:0;margin:0">
    <legend class="muted" style="font-size:.85rem">Canal</legend>
    <div class="choice" role="radiogroup" aria-label="Canal">
      {#each channels as c (c)}
        <label><input type="radio" name="channel" value={c} checked={p.channel === c} onchange={() => (p.channel = c)} /><img src={brand.channel(c)} alt={c} /></label>
      {/each}
    </div>
    <div class="hint">Un perfil <code>stable</code> nunca resuelve releases <code>unstable</code> o <code>nightly</code>.</div>
  </fieldset>

  {#if p.mode === 'drakes-java' || p.mode === 'drakes-rust'}
    <div class="form-grid">
      <div class="field">
        <label for="pf-loader">Loader</label>
        <select id="pf-loader" bind:value={p.loader}>
          {#each loaders as l (l)}
            {@const cell = compat?.loaders.find((x) => x.loader === l)?.cell}
            <option value={l} disabled={cell?.support === 'no'}>
              {l}{cell?.support === 'no' ? ' — no compatible con este modo' : cell?.support === 'planned' ? ' — previsto, no implementado' : ''}
            </option>
          {/each}
        </select>
      </div>
      <div class="field">
        <label for="pf-pack">Modpack (id)</label>
        <input id="pf-pack" value={p.modpack ?? ''} oninput={(e) => (p.modpack = e.currentTarget.value)} disabled={!compat?.modpacks_allowed} />
        {#if !compat?.modpacks_allowed}<div class="hint">{modeLabel[p.mode]} no admite modpacks Java.</div>{/if}
      </div>
    </div>
  {/if}

  {#if p.mode === 'online' || p.mode === 'drakes-java' || p.mode === 'drakes-rust'}
    <div class="field">
      <label for="pf-server">Servidor{p.mode === 'online' ? ' (obligatorio)' : ' (opcional)'}</label>
      <input id="pf-server" value={p.server ?? ''} oninput={(e) => (p.server = e.currentTarget.value)} placeholder="host o host:puerto" />
    </div>
  {/if}
  {#if p.mode === 'local-server'}
    <div class="field">
      <label for="pf-preset">Preset</label>
      <input id="pf-preset" value={p.preset ?? ''} oninput={(e) => (p.preset = e.currentTarget.value)} />
      <div class="hint">El preset Slimefun todavía no copia plugins automáticamente (pendiente).</div>
    </div>
  {/if}

  {#if p.mode !== 'online'}
    <div class="form-grid">
      <div class="field"><label for="pf-min">Memoria mínima (MiB)</label><input id="pf-min" type="number" min="128" step="128" bind:value={p.java.min_mem_mb} /></div>
      <div class="field"><label for="pf-max">Memoria máxima (MiB)</label><input id="pf-max" type="number" min="128" step="128" bind:value={p.java.max_mem_mb} /></div>
      <div class="field"><label for="pf-jv">Java (major, vacío = deducir)</label><input id="pf-jv" type="number" min="8" bind:value={p.java.major} /></div>
    </div>
    <div class="field"><label for="pf-args">Argumentos JVM extra</label><input id="pf-args" bind:value={jvmArgs} placeholder="-XX:+UseZGC" /></div>
  {/if}

  {#if p.mode === 'drakes-java' || p.mode === 'drakes-rust'}
    <fieldset style="border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; padding: 0.75rem 1rem; margin-top: 0.5rem; background: rgba(0,0,0,0.2);">
      <legend style="padding: 0 0.5rem; font-weight: 600; color: var(--gold-2, #f59e0b);">🎮 Gráficos & Pantalla (OpenGL / Vulkan)</legend>
      <div class="form-grid">
        <div class="field">
          <label for="pf-gfx-backend">Pipeline de Renderizado</label>
          <select id="pf-gfx-backend" bind:value={graphicsBackend}>
            <option value="opengl">OpenGL (Estándar / Sodium / Iris Shaders)</option>
            <option value="vulkan">Vulkan (VulkanMod / Zero Overhead)</option>
          </select>
        </div>
        <div class="field">
          <label for="pf-gfx-w">Resolución de Ventana</label>
          <div class="row" style="gap: 0.5rem; align-items: center;">
            <input id="pf-gfx-w" type="number" min="640" step="40" placeholder="1280" bind:value={graphicsWidth} style="width: 45%;" />
            <span>×</span>
            <input id="pf-gfx-h" type="number" min="480" step="40" placeholder="720" bind:value={graphicsHeight} style="width: 45%;" />
          </div>
        </div>
      </div>
      <div class="row" style="gap: 1.5rem; margin-top: 0.5rem; flex-wrap: wrap;">
        <label class="row" style="gap: 0.4rem; cursor: pointer; align-items: center;">
          <input type="checkbox" bind:checked={graphicsFullscreen} />
          <span>Pantalla completa (Fullscreen)</span>
        </label>
        <label class="row" style="gap: 0.4rem; cursor: pointer; align-items: center;">
          <input type="checkbox" bind:checked={graphicsDiscreteGpu} />
          <span>Forzar GPU dedicada (PRIME / NVIDIA)</span>
        </label>
      </div>
    </fieldset>
  {/if}

  <div class="field"><label for="pf-notes">Notas</label><textarea id="pf-notes" rows="2" bind:value={p.notes}></textarea></div>

  <details>
    <summary>Compatibilidades de {modeLabel[p.mode]}</summary>
    <CompatTable mode={p.mode} />
  </details>

  {#if serverError}<div class="alert err" role="alert">Validación (drakestar-core): {serverError}</div>{:else}<div class="alert info">Perfil válido según drakestar-core.</div>{/if}

  <div class="row">
    <button class="btn primary" type="submit" disabled={saving || !!serverError}>{creating ? 'Crear perfil' : 'Guardar'}</button>
    <button class="btn" type="button" onclick={oncancel}>Cancelar</button>
  </div>
</form>
