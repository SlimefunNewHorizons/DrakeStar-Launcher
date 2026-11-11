import { api, onProcEvent } from './api';
import type { AppInfo, LineRec, ModeCompat, Profile, ProcStatus, Settings, Mode } from './types';

export interface Toast { kind: 'ok' | 'err' | 'info'; text: string }

const CAP = 5000;
const idle = (): ProcStatus => ({ state: 'stopped', pid: null, started_at: null, exit_code: null, log_file: null });

class AppState {
  info = $state<AppInfo | null>(null);
  compat = $state<ModeCompat[]>([]);
  profiles = $state<Profile[]>([]);
  invalid = $state<{ id: string; error: string }[]>([]);
  settings = $state<Settings>({ java_path: null, active_profile: null });
  status = $state<Record<string, ProcStatus>>({});
  consoles = $state<Record<string, LineRec[]>>({});
  toast = $state<Toast | null>(null);
  ready = $state(false);
  crashReport = $state<{
    show: boolean;
    title: string;
    errorLog: string;
    component: string;
  }>({
    show: false,
    title: '',
    errorLog: '',
    component: ''
  });
  private toastTimer: number | undefined;

  get active(): Profile | null {
    return this.profiles.find((p) => p.id === this.settings.active_profile) ?? null;
  }
  compatOf(mode: Mode): ModeCompat | undefined {
    return this.compat.find((c) => c.mode === mode);
  }
  statusOf(id: string): ProcStatus {
    return this.status[id] ?? idle();
  }

  notify(kind: Toast['kind'], text: string) {
    this.toast = { kind, text };
    clearTimeout(this.toastTimer);
    this.toastTimer = window.setTimeout(() => (this.toast = null), kind === 'err' ? 9000 : 4000);
  }

  reportCrash(profile: string, title?: string, reason?: string) {
    const lines = this.consoles[profile] ?? [];
    const tail = lines.slice(-40).map((l) => `[${l.stream}] ${l.line}`).join('\n');
    const p = this.profiles.find((x) => x.id === profile);
    this.crashReport = {
      show: true,
      title: title || `Falla inesperada en perfil ${p?.name || profile}`,
      errorLog: tail || reason || 'No se registraron logs específicos en el búfer.',
      component: p?.mode ? `DrakeStar Mode: ${p.mode}` : 'Minecraft / IntegratedServer'
    };
  }

  async guard<T>(fn: () => Promise<T>, okText?: string): Promise<T | undefined> {
    try {
      const r = await fn();
      if (okText) this.notify('ok', okText);
      return r;
    } catch (e) {
      this.notify('err', e instanceof Error ? e.message : String(e));
      return undefined;
    }
  }

  async init() {
    [this.info, this.compat] = await Promise.all([api.appInfo(), api.compat()]);
    this.settings = await api.getSettings();
    await this.refreshProfiles();
    await onProcEvent((e) => {
      if (e.type === 'line') {
        const q = (this.consoles[e.profile] ??= []);
        q.push({ ts: e.ts, stream: e.stream, line: e.line });
        if (q.length > CAP) q.splice(0, q.length - CAP);
      } else {
        const prev = this.statusOf(e.profile);
        const exitCode = e.code ?? prev.exit_code;
        this.status[e.profile] = { ...prev, state: e.state, exit_code: exitCode };
        // Si el proceso termina con error inesperado (no parada normal código 0, SIGINT 130 o SIGTERM 143)
        if (e.state === 'crashed' || (e.state === 'stopped' && exitCode !== null && exitCode !== 0 && exitCode !== 130 && exitCode !== 143)) {
          this.reportCrash(e.profile, `Proceso detenido inesperadamente (código ${exitCode})`);
        }
      }
    });
    this.ready = true;
  }

  async refreshProfiles() {
    const l = await api.listProfiles();
    this.profiles = l.profiles;
    this.invalid = l.invalid;
    if (this.settings.active_profile && !this.active) await this.setActive(null);
    if (!this.settings.active_profile && this.profiles.length) await this.setActive(this.profiles[0].id);
    for (const p of this.profiles) if (p.mode === 'local-server') void this.refreshStatus(p.id);
  }

  async refreshStatus(id: string) {
    this.status[id] = await api.procStatus(id);
  }

  async loadConsole(id: string) {
    this.consoles[id] = await api.consoleHistory(id);
    await this.refreshStatus(id);
  }

  async setActive(id: string | null) {
    this.settings = await api.setSettings({ ...this.settings, active_profile: id });
  }
}

export const app = new AppState();
