export type Kind = 'microsoft' | 'offline';
export interface Account { id: string; kind: Kind; name: string; uuid: string; added_at: number; last_used?: number | null }
export interface AccountList { accounts: Account[]; default: string | null }

export type Target = { kind: 'menu' } | { kind: 'server'; address: string } | { kind: 'world'; name: string } | { kind: 'local'; port: number; lan: boolean };
export interface Instance {
  id: string; name: string; mc: string; loader: 'vanilla' | 'fabric'; loader_version?: string | null; target: Target;
  performance: boolean; fused: boolean; xmx_mb: number; extra_jvm: string[]; note: string;
}

export interface Settings {
  language: string; azure_client_id: string | null; send_reports: boolean; report_endpoint: string | null;
  device_id: string; last_instance: string | null; auto_update: boolean; first_run_done: boolean;
}

export interface AppInfo {
  version: string; data_dir: string; os: string; arch: string; min_mc: string; max_mc: string; client_id_temporary: boolean;
}

export interface DeviceCode {
  device_code: string; user_code: string; verification_uri: string; verification_uri_complete: string | null;
  interval: number; expires_in: number;
}

export interface Progress { stage: string; text: string; fraction: number }
export type GameStateName = 'preparing' | 'running' | 'exited' | 'crashed' | 'error' | 'cancelled';
export interface GameState { state: GameStateName; message: string; code?: number | null; warnings: string[] }
export interface MsaEvent { phase: 'working' | 'success' | 'error' | 'cancelled'; message: string; account?: Account | null }
export interface ReportInfo { path: string; sent: boolean }

export interface MemoryProfile { total_mb: number; recommended_mb: number; max_mb: number }
export interface ImportPreview { format: string; instance: Instance }
