// Espejo de los tipos serializados por drakestar-core / drakestar-app.
export type Mode = 'drakes-rust' | 'drakes-java' | 'local-server' | 'online';
export type Channel = 'stable' | 'unstable' | 'nightly';
export type Loader = 'vanilla' | 'fabric' | 'forge' | 'neoforge';
export type Support = 'yes' | 'no' | 'planned';

export interface JavaSettings {
  major: number | null;
  min_mem_mb: number;
  max_mem_mb: number;
  extra_jvm_args: string[];
}
export type GraphicsBackend = 'opengl' | 'vulkan';
export interface GraphicsSettings {
  backend: GraphicsBackend;
  width?: number | null;
  height?: number | null;
  fullscreen: boolean;
  discrete_gpu: boolean;
}

export interface Profile {
  schema: number;
  id: string;
  name: string;
  mode: Mode;
  channel: Channel;
  minecraft: string;
  loader: Loader;
  java: JavaSettings;
  graphics?: GraphicsSettings;
  server: string | null;
  preset: string | null;
  modpack: string | null;
  notes: string;
}
export interface ProfileList {
  profiles: Profile[];
  invalid: { id: string; error: string }[];
}
export interface Cell { support: Support; caveat: string | null }
export interface ModeCompat {
  mode: Mode;
  label: string;
  loaders: { loader: Loader; cell: Cell }[];
  allowed_loaders: Loader[];
  modpacks_allowed: boolean;
  server_plugins: Cell;
  slimefun_gui: Cell;
  launch_implemented: boolean;
  launch_note: string;
}
export interface AppInfo {
  name: string;
  version: string;
  data_dir: string;
  platform: string;
  /** 'tauri' = backend Rust real; 'mock' = simulacion en el navegador. */
  backend: 'tauri' | 'mock';
}
export interface Settings { java_path: string | null; active_profile: string | null }
export interface JavaInfo { bin: string; found: boolean; version: string | null; major: number | null; error: string | null }
export interface JavaCheck { info: JavaInfo; warning: string | null }

export type ProcState = 'stopped' | 'running' | 'stopping' | 'crashed';
export interface ProcStatus {
  state: ProcState;
  pid: number | null;
  started_at: number | null;
  exit_code: number | null;
  log_file: string | null;
}
export type StreamKind = 'stdout' | 'stderr' | 'system' | 'stdin';
export interface LineRec { ts: number; stream: StreamKind; line: string }
export type ProcEvent =
  | { type: 'line'; profile: string; ts: number; stream: StreamKind; line: string }
  | { type: 'state'; profile: string; state: ProcState; code: number | null };

export interface InstanceInfo {
  id: string; path: string; data_dir: string; data_exists: boolean; size_bytes: number;
  modified: number | null; server_jar: string | null; launcher_logs: number;
}
export interface InstanceRow { profile: Profile; info: InstanceInfo; status: ProcStatus }
export interface LogFile { source: 'launcher' | 'server'; name: string; size: number; modified: number | null }

export interface ServerStatus {
  dir: string; exists: boolean;
  jar: { selected: string | null; candidates: string[] };
  eula: boolean | null; has_properties: boolean; plugins: number; worlds: number;
}
export interface Prop { key: string; value: string }
export interface PluginInfo { file: string; name: string; enabled: boolean; size: number }
export interface WorldInfo { name: string; size: number; modified: number | null }

export interface Resolution {
  profile: string;
  profile_channel: Channel;
  component: string | null;
  platform: string;
  minecraft: string;
  resolved: null | {
    version: string; release_channel: Channel; url: string; sha256: string; size: number;
    sha256_wellformed: boolean; notes: string;
  };
  reason: string | null;
}

export interface ModEntry {
  file: string;
  id: string;
  version: string;
  sha256: string;
  size: number;
  url: string;
  required?: boolean;
  depends?: string[];
  conflicts?: string[];
  mixin_targets?: string[];
}

export interface DeltaSyncPlan {
  unchanged: string[];
  to_download: ModEntry[];
  to_remove: string[];
  total_download_bytes: number;
}

export interface DeltaSyncResult {
  downloaded: string[];
  removed: string[];
  unchanged: string[];
  bytes_downloaded: number;
}

export interface ConflictReport {
  compatible: boolean;
  mods_analyzed: number;
  missing_dependencies: string[];
  explicit_conflicts: string[];
  duplicate_ids: string[];
  mixin_collisions: string[];
}

export interface McStatus {
  latency_ms: number;
  version_name: string | null;
  protocol: number | null;
  players_online: number | null;
  players_max: number | null;
  description: string | null;
  favicon?: string | null;
  raw_json?: string | null;
}

/** Estado del cliente Rust nativo (`drakestar_app::rustclient`). */
export interface RustClientStatus {
  profile: string;
  binary: string;
  installed: boolean;
  executable: boolean;
  version: string | null;
  ready: boolean;
  reason: string | null;
}

/** Informe del cargador de mods (`drakestar_app::mods`). */
export interface ScanResult {
  file: string;
  risk: 'safe' | 'suspicious' | 'dangerous';
  reasons: string[];
  suspicious_strings: string[];
  quarantine: boolean;
  user_override: boolean;
}

export interface LoadReport {
  discovered: number;
  verified: number;
  /** ids en orden de carga (dependencias primero). */
  order: string[];
  /** id -> motivo por el que no carga. */
  not_loaded: [string, string][];
  /** id -> runtime, ABI. */
  abi: [string, string, string][];
  /** jars a agregar al classpath, en orden. */
  classpath: string[];
  warnings: [string, string][];
  bytes: number;
  /** jars que no eran mods de Fabric validos. */
  skipped: string[];
  security_alerts?: ScanResult[];
}

export interface ModrinthSearchHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  categories: string[];
  icon_url: string | null;
  downloads: number;
  follows: number;
  author: string;
}

export interface ModrinthVersionFile {
  url: string;
  filename: string;
  primary: boolean;
  size: number;
}

export interface ModrinthVersion {
  id: string;
  name: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  files: ModrinthVersionFile[];
}

export interface PrismInstanceSummary {
  dir_name: string;
  path: string;
  name: string;
  icon: string;
  minecraft_version: string;
  loader_version: string | null;
  mod_count: number;
}

export interface PrismImportResult {
  profile_id: string;
  profile_name: string;
  mods_imported: number;
}

/** Estado del cliente Java gestionado (Fabric) (`drakestar_app::fabric`). */
export interface ClientStatus {
  profile: string;
  minecraft: string;
  java_required: number;
  loader_version: string | null;
  main_class: string | null;
  resolved: boolean;
  artifacts: number;
  present: number;
  bytes: number;
  ready: boolean;
  reason: string | null;
}
