import { t } from './i18n.svelte';
import type { Instance } from './types';

export const loaderLabel = (i: Instance) => (i.loader === 'fabric' ? 'Fabric' : 'Vanilla');

export function targetLabel(i: Instance): string {
  switch (i.target.kind) {
    case 'server': return t('target.server', { addr: i.target.address });
    case 'world': return t('target.world', { name: i.target.name });
    case 'local': return t('target.local');
    default: return t('target.menu');
  }
}

export const fmtRam = (mb: number) => `${(mb / 1024).toFixed(mb % 1024 === 0 ? 0 : 1)} GB`;

export function slug(name: string): string {
  const base = name.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 38);
  return `${base || 'instancia'}-${Math.random().toString(36).slice(2, 6)}`;
}

export const validOfflineName = (n: string) => /^[A-Za-z0-9_]{3,16}$/.test(n);

export function errText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  try { return JSON.stringify(e); } catch { return String(e); }
}

export function lineClass(l: string): string {
  if (/\b(ERROR|FATAL|PANIC|Exception|Caused by)\b/.test(l) || l.includes('[err]')) return 'err';
  if (/\bWARN\b/.test(l)) return 'warn';
  return '';
}
