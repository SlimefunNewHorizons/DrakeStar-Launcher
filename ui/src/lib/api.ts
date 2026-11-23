// Capa de acceso al backend. Dentro de la app usa Tauri; en el navegador (npm run dev) usa datos de ejemplo
// para poder revisar la interfaz sin compilar Rust.
import type { CatalogEntry, CatalogHit, ContentKind, ImportPreview, MemoryProfile, Account, AccountList, AppInfo, DeviceCode, GameState, Instance, MsaEvent, Progress, ReportInfo, Settings } from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

type Listener<T> = (p: T) => void;
const bus = new Map<string, Set<Listener<any>>>();
function emitMock(ev: string, payload: unknown) { bus.get(ev)?.forEach((l) => l(payload)); }

export async function listen<T>(event: string, cb: Listener<T>): Promise<() => void> {
  if (inTauri) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(event, (e) => cb(e.payload));
  }
  if (!bus.has(event)) bus.set(event, new Set());
  bus.get(event)!.add(cb);
  return () => bus.get(event)?.delete(cb);
}

async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }
  return mock<T>(cmd, args);
}

export const api = {
  catalogSearch: (instance: string, kind: ContentKind, query: string) => call<CatalogHit[]>('catalog_search', { instance, kind, query }),
  catalogList: (instance: string) => call<CatalogEntry[]>('catalog_list', { instance }),
  catalogChange: (instance: string, kind: ContentKind, project: string, remove: boolean) => call<CatalogEntry[]>('catalog_change', { instance, kind, project, remove }),
  appInfo: () => call<AppInfo>('app_info'),
  settingsGet: () => call<Settings>('settings_get'),
  settingsSet: (settings: Settings) => call<Settings>('settings_set', { settings }),
  accounts: () => call<AccountList>('accounts_list'),
  addElyby: (username: string, password: string, totp: string) => call<Account>('account_add_elyby', { username, password, totp }),
  addOffline: (name: string) => call<Account>('account_add_offline', { name }),
  removeAccount: (id: string) => call<void>('account_remove', { id }),
  setDefaultAccount: (id: string) => call<void>('account_set_default', { id }),
  msaStart: () => call<DeviceCode>('msa_start'),
  msaCancel: () => call<void>('msa_cancel'),
  memoryProfile: () => call<MemoryProfile>('memory_profile'),
  importPreview: (source: string) => call<ImportPreview>('instance_import_preview', { source }),
  importInstance: (source: string) => call<Instance>('instance_import', { source }),
  instances: () => call<Instance[]>('instances_list'),
  saveInstance: (instance: Instance) => call<Instance>('instance_save', { instance }),
  deleteInstance: (id: string) => call<void>('instance_delete', { id }),
  versions: () => call<string[]>('versions_supported'),
  play: (instanceId: string, accountId?: string) => call<void>('play', { instanceId, accountId }),
  playCancel: () => call<void>('play_cancel'),
  gameKill: () => call<void>('game_kill'),
  logs: (source: 'launcher' | 'game', instance: string | null, maxLines = 400) => call<string[]>('logs_read', { source, instance, maxLines }),
  report: (instance: string | null, send: boolean) => call<ReportInfo>('support_report', { instance, send }),
  openFolder: (kind: 'instance' | 'logs' | 'support' | 'data', id?: string) => call<void>('open_folder', { kind, id }),
  openUrl: (url: string) => call<void>('open_url', { url }),
};

// ---------- actualizaciones (plugin de Tauri) ----------
export interface UpdateInfo { version: string; notes: string | null; install: () => Promise<void> }
const query = typeof location !== 'undefined' ? location.search : '';
export async function checkUpdate(): Promise<UpdateInfo | null> {
  if (!inTauri) return query.includes('update') ? { version: '0.7.0', notes: null, install: async () => {} } : null;
  const { check } = await import('@tauri-apps/plugin-updater');
  const u = await check();
  if (!u) return null;
  return {
    version: u.version, notes: u.body ?? null,
    install: async () => {
      await u.downloadAndInstall();
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    },
  };
}

// ---------- datos de ejemplo del modo navegador (?fresh = primer arranque, ?update = hay version nueva) ----------
const fresh = query.includes('fresh');
const mockState = {
  accounts: fresh ? ({ accounts: [], default: null } as AccountList) : { accounts: [{ id: 'msa-1', kind: 'microsoft', name: 'JackStar6677', uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5', added_at: 1, last_used: 2 }, { id: 'offline-1', kind: 'offline', name: 'Invitado', uuid: 'b50ad385-829d-3141-a216-7e7d7539ba7f', added_at: 1 }], default: 'msa-1' } as AccountList,
  instances: [
    { id: 'drakescraft', name: 'DrakesCraft (online)', mc: '1.21.11', loader: 'fabric', target: { kind: 'server', address: 'mc.drakescraft.cl:25565' }, performance: true, fused: false, xmx_mb: 4096, extra_jvm: [], note: 'Entra directo al servidor oficial.' },
    { id: 'slimefun-solo', name: 'Slimefun en solitario / LAN', mc: '1.21.1', loader: 'fabric', target: { kind: 'menu' }, performance: true, fused: true, xmx_mb: 4096, extra_jvm: [], note: 'Mundo propio con Slimefun.' },
    { id: 'vanilla-latest', name: 'Minecraft vanilla', mc: '1.21.11', loader: 'vanilla', target: { kind: 'menu' }, performance: false, fused: false, xmx_mb: 3072, extra_jvm: [], note: 'Sin mods.' },
  ] as Instance[],
  settings: { language: 'es', azure_client_id: null, send_reports: true, report_endpoint: null, device_id: 'demo', last_instance: 'drakescraft', auto_update: true, first_run_done: !fresh } as Settings,
};

async function mock<T>(cmd: string, a: Record<string, any>): Promise<T> {
  const ok = (v: unknown) => Promise.resolve(v as T);
  switch (cmd) {
    case 'catalog_list': return ok([]);
    case 'catalog_search':
    case 'catalog_change': throw new Error('El catálogo requiere la aplicación de escritorio.');
    case 'app_info': return ok({ version: '0.6.0', data_dir: 'C:\\Users\\demo\\AppData\\Roaming\\net.drakes.drakestar', os: 'windows', arch: 'x86_64', min_mc: '1.20', max_mc: '26.2', client_id_temporary: true });
    case 'settings_get': return ok(mockState.settings);
    case 'settings_set': mockState.settings = a.settings; return ok(a.settings);
    case 'accounts_list': return ok(mockState.accounts);
    case 'account_add_elyby': throw new Error('Ely.by requiere la aplicación de escritorio.');
    case 'account_add_offline': { const acc = { id: 'offline-' + a.name, kind: 'offline', name: a.name, uuid: 'x', added_at: 3 } as Account; mockState.accounts.accounts.push(acc); return ok(acc); }
    case 'account_remove': mockState.accounts.accounts = mockState.accounts.accounts.filter((x) => x.id !== a.id); return ok(undefined);
    case 'account_set_default': mockState.accounts.default = a.id; return ok(undefined);
    case 'msa_start': {
      setTimeout(() => emitMock('msa', { phase: 'success', message: 'Sesion iniciada como JackStar6677', account: mockState.accounts.accounts[0] } satisfies MsaEvent), 4000);
      return ok({ device_code: 'x', user_code: 'ABCD1234', verification_uri: 'https://www.microsoft.com/link', verification_uri_complete: null, interval: 5, expires_in: 900 });
    }
    case 'msa_cancel': return ok(undefined);
    case 'memory_profile': return ok({ total_mb: 16384, recommended_mb: 6144, max_mb: 8192 });
    case 'instance_import_preview':
    case 'instance_import': throw new Error('La importación de archivos requiere la aplicación de escritorio.');
    case 'instances_list': return ok(mockState.instances);
    case 'instance_save': { const i = mockState.instances.findIndex((x) => x.id === a.instance.id); if (i >= 0) mockState.instances[i] = a.instance; else mockState.instances.push(a.instance); return ok(a.instance); }
    case 'instance_delete': mockState.instances = mockState.instances.filter((x) => x.id !== a.id); return ok(undefined);
    case 'versions_supported': return ok(['26.2', '26.1', '1.21.11', '1.21.10', '1.21.8', '1.21.5', '1.21.4', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.2', '1.20.1', '1.20']);
    case 'play': {
      emitMock('game-state', { state: 'preparing', message: 'Preparando', warnings: [] } satisfies GameState);
      const steps: Progress[] = [{ stage: 'java', text: 'Descargando Java 21 (solo la primera vez)', fraction: 0.4 }, { stage: 'minecraft', text: 'Preparando Minecraft', fraction: 0.7 }, { stage: 'assets', text: 'Descargando sonidos y recursos', fraction: 0.9 }];
      steps.forEach((p, i) => setTimeout(() => emitMock('progress', p), 600 * (i + 1)));
      setTimeout(() => emitMock('game-state', { state: 'running', message: '', warnings: [] } satisfies GameState), 2600);
      return ok(undefined);
    }
    case 'logs_read': return ok(['[INFO] DrakeStar arranca (windows x86_64)', '[INFO] [java] Descargando Java 21', '[INFO] lanzando drakescraft con Java 21']);
    case 'support_report': return ok({ path: 'C:\\Users\\demo\\support\\report.zip', sent: a.send });
    default: return ok(undefined);
  }
}
