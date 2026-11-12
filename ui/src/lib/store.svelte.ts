// Estado global de la interfaz. Toda la logica de juego vive en Rust; aqui solo se refleja lo que pasa.
import { api, checkUpdate, listen, type UpdateInfo } from './api';
import { errText } from './format';
import { setLang, t } from './i18n.svelte';
import type { Account, AccountList, AppInfo, DeviceCode, GameState, GameStateName, Instance, MsaEvent, Progress, ReportInfo, Settings } from './types';

export type View = 'play' | 'accounts' | 'instances' | 'settings' | 'logs';
export type Phase = 'idle' | GameStateName;
export interface Toast { id: number; kind: 'info' | 'ok' | 'error'; text: string }
export interface MsaUi { open: boolean; step: 'starting' | 'waiting' | 'working' | 'error'; code: DeviceCode | null; message: string }

const MAX_LINES = 400;

class Store {
  view = $state<View>('play');
  ready = $state(false);
  info = $state<AppInfo | null>(null);
  settings = $state<Settings | null>(null);
  accounts = $state<AccountList>({ accounts: [], default: null });
  instances = $state<Instance[]>([]);
  versions = $state<string[]>([]);
  selected = $state<string | null>(null);

  phase = $state<Phase>('idle');
  message = $state('');
  exitCode = $state<number | null>(null);
  warnings = $state<string[]>([]);
  progress = $state<Progress | null>(null);
  gameLines = $state<string[]>([]);
  report = $state<ReportInfo | null>(null);

  update = $state<UpdateInfo | null>(null);
  updateState = $state<'idle' | 'checking' | 'none' | 'installing' | 'error'>('idle');
  toasts = $state<Toast[]>([]);
  msa = $state<MsaUi>({ open: false, step: 'starting', code: null, message: '' });

  private nextToast = 1;

  get instance(): Instance | null {
    return this.instances.find((i) => i.id === this.selected) ?? this.instances[0] ?? null;
  }
  get account(): Account | null {
    const l = this.accounts;
    return l.accounts.find((a) => a.id === l.default) ?? l.accounts[0] ?? null;
  }
  get busy(): boolean {
    return this.phase === 'preparing' || this.phase === 'running';
  }

  toast(kind: Toast['kind'], text: string) {
    const id = this.nextToast++;
    this.toasts.push({ id, kind, text });
    setTimeout(() => this.dismiss(id), kind === 'error' ? 9000 : 4500);
  }
  dismiss(id: number) {
    this.toasts = this.toasts.filter((x) => x.id !== id);
  }

  async init() {
    await Promise.all([
      listen<Progress>('progress', (p) => (this.progress = p)),
      listen<GameState>('game-state', (g) => this.onGameState(g)),
      listen<{ stream: string; line: string }>('game-line', (l) => this.pushLine(l.line)),
      listen<MsaEvent>('msa', (m) => this.onMsa(m)),
      listen<ReportInfo>('report', (r) => (this.report = r)),
    ]);
    try {
      const [info, settings] = await Promise.all([api.appInfo(), api.settingsGet()]);
      this.info = info;
      this.settings = settings;
      setLang(settings.language);
      await Promise.all([this.loadAccounts(), this.loadInstances()]);
      if (settings.last_instance && this.instances.some((i) => i.id === settings.last_instance)) this.selected = settings.last_instance;
    } catch (e) {
      this.toast('error', errText(e));
    }
    this.ready = true;
    this.loadVersions();
    if (this.settings?.auto_update) setTimeout(() => this.checkUpdates(false), 2500);
  }

  async loadAccounts() {
    this.accounts = await api.accounts();
  }
  async loadInstances() {
    this.instances = await api.instances();
  }
  async loadVersions() {
    try { this.versions = await api.versions(); } catch { /* sin red: el formulario deja escribir la version a mano */ }
  }

  async saveSettings(patch: Partial<Settings>, quiet = true) {
    if (!this.settings) return;
    const next = { ...this.settings, ...patch };
    try {
      this.settings = await api.settingsSet(next);
      if (patch.language) setLang(patch.language);
      if (!quiet) this.toast('ok', t('set.saved'));
      if ('azure_client_id' in patch) this.info = await api.appInfo();
    } catch (e) {
      this.toast('error', errText(e));
    }
  }

  // ---------- cuentas ----------
  async addOffline(name: string): Promise<boolean> {
    try {
      await api.addOffline(name);
      await this.loadAccounts();
      return true;
    } catch (e) {
      this.toast('error', errText(e));
      return false;
    }
  }
  async removeAccount(id: string) {
    try { await api.removeAccount(id); await this.loadAccounts(); } catch (e) { this.toast('error', errText(e)); }
  }
  async useAccount(id: string) {
    try { await api.setDefaultAccount(id); await this.loadAccounts(); } catch (e) { this.toast('error', errText(e)); }
  }

  async startMsa() {
    this.msa = { open: true, step: 'starting', code: null, message: '' };
    try {
      await api.msaCancel();
      const code = await api.msaStart();
      this.msa = { open: true, step: 'waiting', code, message: '' };
    } catch (e) {
      this.msa = { open: true, step: 'error', code: null, message: errText(e) };
    }
  }
  async cancelMsa() {
    this.msa.open = false;
    try { await api.msaCancel(); } catch { /* ya terminado */ }
  }
  private onMsa(m: MsaEvent) {
    if (!this.msa.open) return;
    if (m.phase === 'working') this.msa = { ...this.msa, step: 'working', message: m.message };
    else if (m.phase === 'success') {
      this.msa.open = false;
      this.loadAccounts();
      this.toast('ok', m.message);
    } else if (m.phase === 'error') this.msa = { ...this.msa, step: 'error', message: m.message };
    else this.msa.open = false;
  }

  // ---------- instancias ----------
  select(id: string) {
    this.selected = id;
  }
  async saveInstance(i: Instance): Promise<string | null> {
    try {
      const saved = await api.saveInstance(i);
      await this.loadInstances();
      this.selected = saved.id;
      return null;
    } catch (e) {
      return errText(e);
    }
  }
  /** Tras un fallo en una instancia con Fused: lo apaga (y borra su jar al jugar de nuevo). */
  async disableFused() {
    const i = this.instance;
    if (!i) return;
    const err = await this.saveInstance({ ...$state.snapshot(i), fused: false });
    this.toast(err ? 'error' : 'ok', err ?? t('set.saved'));
  }
  async deleteInstance(id: string) {
    try {
      await api.deleteInstance(id);
      await this.loadInstances();
      if (this.selected === id) this.selected = null;
    } catch (e) {
      this.toast('error', errText(e));
    }
  }

  // ---------- jugar ----------
  async play() {
    const inst = this.instance;
    if (!inst || this.busy) return;
    this.phase = 'preparing';
    this.message = '';
    this.exitCode = null;
    this.warnings = [];
    this.progress = null;
    this.gameLines = [];
    this.report = null;
    try {
      await api.play(inst.id, this.account?.id);
    } catch (e) {
      this.phase = 'error';
      this.message = errText(e);
    }
  }
  cancelPlay() { api.playCancel().catch(() => {}); }
  killGame() { api.gameKill().catch(() => {}); }

  private onGameState(g: GameState) {
    this.phase = g.state;
    this.message = g.message;
    this.exitCode = g.code ?? null;
    this.warnings = g.warnings ?? [];
    if (g.state !== 'preparing') this.progress = null;
    if (g.state === 'preparing') this.report = null;
  }
  private pushLine(line: string) {
    this.gameLines.push(line);
    if (this.gameLines.length > MAX_LINES) this.gameLines.splice(0, this.gameLines.length - MAX_LINES);
  }

  // ---------- soporte ----------
  async makeReport(send: boolean) {
    try {
      const r = await api.report(this.instance?.id ?? null, send);
      this.report = r;
      this.toast('ok', r.sent ? t('log.sent') : t('log.saved', { path: r.path }));
    } catch (e) {
      this.toast('error', errText(e));
    }
  }

  // ---------- actualizaciones ----------
  async checkUpdates(manual: boolean) {
    this.updateState = 'checking';
    try {
      this.update = await checkUpdate();
      this.updateState = this.update ? 'idle' : 'none';
      if (manual && !this.update) this.toast('ok', t('set.uptodate'));
    } catch (e) {
      this.updateState = 'error';
      if (manual) this.toast('error', t('set.check_failed'));
      console.warn('update check failed:', errText(e));
    }
  }
  async installUpdate() {
    if (!this.update || this.busy) return;
    this.updateState = 'installing';
    try {
      await this.update.install();
    } catch (e) {
      this.updateState = 'error';
      this.toast('error', errText(e));
    }
  }
}

export const app = new Store();
