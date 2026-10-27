import type { Clip } from './types';

export interface PrivacyRule { id: string; name: string; enabled: boolean; condition: 'keyword' | 'domain'; value: string; tag: string | null; markPrivate: boolean }
export interface PrivacyRules { revision: number; rules: PrivacyRule[] }
export interface PrivacyScan { token: string; total: number; privateCount: number; newlyPrivateCount: number; changedCount: number }
export const sourceNames: Record<string, string> = { wechat: '微信公众号', linuxdo: 'LINUX DO', x: 'X', 'x-short': 'X 短链 · 未解析', bilibili: '哔哩哔哩', 'bilibili-short': '哔哩哔哩短链 · 未解析' };
export const protectedClip = (clip: Clip) => !!clip.isPrivate || !!clip.privacyManual || !!clip.privacyReasons?.length;
export const pendingClip = (clip: Clip, revision: number) => !clip.scanVersion || clip.scanVersion !== revision;
export function matchesMetadata(clip: Clip, sources: string[], tag: string, privacy: string, revision: number): boolean {
  return (!sources.length || sources.some(source => source.startsWith('host:') ? clip.linkHosts?.includes(source.slice(5)) : clip.linkSources?.includes(source)))
    && (!tag || !!clip.tags?.includes(tag))
    && (privacy === 'all' || (privacy === 'private' ? protectedClip(clip) : privacy === 'pending' ? pendingClip(clip, revision) : !protectedClip(clip) && !pendingClip(clip, revision)));
}
export function normalizeTags(input: string): string[] { return [...new Set(input.split(/[,，\n]/).map(value => value.trim()).filter(Boolean))]; }

// Browser fixtures only. Desktop policy and complete sensitive detection live in Rust.
export function analyzeDemoClip(clip: Clip, config: PrivacyRules): Clip {
  const hosts = new Set<string>(), sources = new Set<string>(), reasons = new Set(clip.privacyReasons ?? []), tags = new Set(clip.manualTags ?? clip.tags ?? []);
  const content = `${clip.title}\n${clip.content}`;
  for (const raw of content.match(/https?:\/\/[^\s<>"']+/gi) ?? []) {
    try {
      const url = new URL(raw.replace(/[),.;，。；）]+$/, '')), host = url.hostname.toLowerCase().replace(/\.$/, '');
      hosts.add(host);
      const domain = (value: string) => host === value || host.endsWith(`.${value}`);
      if (host === 'mp.weixin.qq.com') sources.add('wechat');
      if (domain('linux.do')) sources.add('linuxdo');
      if (domain('x.com') || domain('twitter.com')) sources.add('x');
      if (host === 't.co') sources.add('x-short');
      if (domain('bilibili.com')) sources.add('bilibili');
      if (host === 'b23.tv') sources.add('bilibili-short');
      if (url.username || url.password || [...url.searchParams.keys()].some(key => /^(access_token|password|secret|api_key|token)$/i.test(key))) reasons.add('敏感链接');
    } catch { /* Invalid URLs are not classified. */ }
  }
  if (/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(content)) reasons.add('私钥格式');
  if (/(?:password|api_key|secret)\s*[:=]\s*\S{8,}|Bearer\s+[\w.-]{12,}/i.test(content)) reasons.add('疑似凭据');
  for (const rule of config.rules.filter(rule => rule.enabled)) {
    if (rule.condition === 'keyword' ? content.toLocaleLowerCase().includes(rule.value.toLocaleLowerCase()) : hosts.has(rule.value.toLowerCase().replace(/\.$/, ''))) {
      if (rule.tag) tags.add(rule.tag);
      if (rule.markPrivate) reasons.add(rule.name);
    }
  }
  return { ...clip, privacyManual: !!clip.privacyManual, isPrivate: !!clip.privacyManual || reasons.size > 0, privacyReasons: [...reasons], scanVersion: config.revision, tags: [...tags], manualTags: [...(clip.manualTags ?? clip.tags ?? [])], linkHosts: [...hosts], linkSources: [...sources] };
}
