// Capa de acceso al backend. En Tauri llama a los comandos reales (drakestar-app).
// En un navegador normal (vite dev / capturas) usa un MOCK en memoria, claramente marcado
// en la UI (AppInfo.backend === 'mock'): no ejecuta procesos ni toca el disco.
import type { AppInfo, JavaCheck, ModeCompat, Profile, ProfileList, ProcEvent, ProcStatus, LineRec, Settings, InstanceRow, LogFile, ServerStatus, Prop, PluginInfo, WorldInfo, Resolution, DeltaSyncPlan, DeltaSyncResult, ConflictReport, McStatus, LoadReport, RustClientStatus, ClientStatus, ModrinthSearchHit, ModrinthVersion, PrismInstanceSummary, PrismImportResult, DeviceCodeAuth, PollResult } from './types';

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = (cb: (e: ProcEvent) => void) => Promise<() => void>;

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

let impl: { invoke: Invoke; listen: Listen } | null = null;
async function load() {
  if (impl) return impl;
  if (isTauri) {
    const core = await import('@tauri-apps/api/core');
    const ev = await import('@tauri-apps/api/event');
    impl = {
      invoke: (cmd, args) => core.invoke(cmd, args),
      listen: (cb) => ev.listen<ProcEvent>('proc-event', (e) => cb(e.payload)),
    };
  } else {
    const m = await import('./mock');
    impl = { invoke: m.mockInvoke as Invoke, listen: m.mockListen };
  }
  return impl;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const i = await load();
  try {
    return await i.invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e));
  }
}
export async function onProcEvent(cb: (e: ProcEvent) => void): Promise<() => void> {
  return (await load()).listen(cb);
}

export const api = {
  appInfo: () => call<AppInfo>('app_info'),
  compat: () => call<ModeCompat[]>('compat_report'),
  listProfiles: () => call<ProfileList>('list_profiles'),
  validateProfile: (profile: Profile) => call<string | null>('validate_profile', { profile }),
  createProfile: (profile: Profile) => call<Profile>('create_profile', { profile }),
  updateProfile: (profile: Profile) => call<Profile>('update_profile', { profile }),
  deleteProfile: (id: string, removeData: boolean) => call<void>('delete_profile', { id, removeData }),
  duplicateProfile: (id: string, newId: string, newName: string) => call<Profile>('duplicate_profile', { id, newId, newName }),
  importProfile: (json: string, overwrite: boolean) => call<Profile>('import_profile', { json, overwrite }),
  exportProfile: (id: string) => call<string>('export_profile', { id }),
  exportProfileFile: (id: string) => call<string>('export_profile_file', { id }),
  getSettings: () => call<Settings>('get_settings'),
  setSettings: (settings: Settings) => call<Settings>('set_settings', { settings }),
  javaCheck: (id: string) => call<JavaCheck>('java_check', { id }),
  listInstances: () => call<InstanceRow[]>('list_instances'),
  ensureInstance: (id: string) => call<void>('ensure_instance', { id }),
  listLogs: (id: string) => call<LogFile[]>('list_logs', { id }),
  readLog: (id: string, source: string, name: string, maxLines: number) => call<string[]>('read_log', { id, source, name, maxLines }),
  clearLogs: (id: string) => call<number>('clear_logs', { id }),
  serverStart: (id: string) => call<ProcStatus>('server_start', { id }),
  serverStop: (id: string) => call<void>('server_stop', { id }),
  serverKill: (id: string) => call<void>('server_kill', { id }),
  serverCommand: (id: string, line: string) => call<void>('server_command', { id, line }),
  procStatus: (id: string) => call<ProcStatus>('proc_status', { id }),
  consoleHistory: (id: string) => call<LineRec[]>('console_history', { id }),
  consoleClear: (id: string) => call<void>('console_clear', { id }),
  serverPlan: (id: string) => call<string>('server_plan', { id }),
  serverStatus: (id: string) => call<ServerStatus>('server_status', { id }),
  acceptEula: (id: string) => call<void>('server_accept_eula', { id }),
  serverProperties: (id: string) => call<Prop[]>('server_properties', { id }),
  setServerProperties: (id: string, updates: Record<string, string>) => call<Prop[]>('server_set_properties', { id, updates }),
  plugins: (id: string) => call<PluginInfo[]>('server_plugins', { id }),
  togglePlugin: (id: string, file: string) => call<string>('server_toggle_plugin', { id, file }),
  worlds: (id: string) => call<WorldInfo[]>('server_worlds', { id }),
  backupWorld: (id: string, worldName: string) => call<string>('server_backup_world', { id, worldName }),
  manifestImport: (json: string) => call<number>('manifest_import', { json }),
  manifestSummary: () => call<number | null>('manifest_summary'),
  resolveRelease: (id: string) => call<Resolution>('resolve_release', { id }),
  installServerJar: (id: string) => call<number>('install_server_jar', { id }),
  modpackDeltaPlan: (id: string, manifestJson: string) => call<DeltaSyncPlan>('modpack_delta_plan', { id, manifestJson }),
  modpackDeltaApply: (id: string, manifestJson: string) => call<DeltaSyncResult>('modpack_delta_apply', { id, manifestJson }),
  analyzeFabricConflicts: (manifestJson: string) => call<ConflictReport>('analyze_fabric_conflicts', { manifestJson }),

  ping: (address: string) => call<number>('ping_server', { address }),
  pingMcStatus: (address: string) => call<McStatus>('ping_mc_status', { address }),
  diagnostics: () => call<string>('diagnostics'),
  modsReport: (id: string) => call<LoadReport>('mods_report', { id }),
  modsDecide: (id: string) => call<LoadReport>('mods_decide', { id }),
  rustStatus: (id: string) => call<RustClientStatus>('rust_client_status', { id }),
  rustPlan: (id: string) => call<string>('rust_client_plan', { id }),
  rustInstall: (id: string) => call<RustClientStatus>('rust_client_install', { id }),
  rustStart: (id: string) => call<ProcStatus>('rust_client_start', { id }),
  rustStop: (id: string) => call<void>('rust_client_stop', { id }),
  rustCommand: (id: string, line: string) => call<void>('rust_client_command', { id, line }),

  clientStatus: (id: string) => call<ClientStatus>('client_status', { id }),
  clientInstall: (id: string) => call<ClientStatus>('client_install', { id }),
  clientPlan: (id: string) => call<string>('client_plan', { id }),
  clientStart: (id: string) => call<ProcStatus>('client_start', { id }),
  clientStop: (id: string) => call<void>('client_stop', { id }),

  modsGuardOverride: (id: string, item: string, allow: boolean) => call<void>('mods_guard_override', { id, item, allow }),
  modrinthSearch: (query: string, limit?: number) => call<ModrinthSearchHit[]>('modrinth_search', { query, limit }),
  modrinthVersions: (projectId: string, mcVersion?: string) => call<ModrinthVersion[]>('modrinth_versions', { projectId, mcVersion }),
  modrinthInstall: (id: string, fileUrl: string, fileName: string) => call<number>('modrinth_install', { id, fileUrl, fileName }),
  prismListInstances: () => call<PrismInstanceSummary[]>('prism_list_instances'),
  prismImportInstance: (path: string) => call<PrismImportResult>('prism_import_instance', { path }),

  msftLoginStart: () => call<DeviceCodeAuth>('msft_login_start'),
  msftLoginPoll: (deviceCode: string) => call<PollResult>('msft_login_poll', { deviceCode }),
  msftLogout: () => call<Settings>('msft_logout'),
};
