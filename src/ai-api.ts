export interface AiConnection { id: string; name: string; recordCount: number; fields: string[]; maxResults: number; expiresAt: number; allowPetControl: boolean }
export interface AiState { enabled: boolean; available: boolean; connections: AiConnection[]; audit: Array<{ connectionName: string; action: string; at: number; count: number; result: string }>; limits: { maxRecords: number; maxResults: number; maxHours: number } }
export interface AiConfig { mcp: object; cli: string; skill: string }
export interface AiRequest { name: string; clipIds: number[]; fields: string[]; maxResults: number; expiresHours: number; allowPetControl: boolean }
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!('__TAURI_INTERNALS__' in window)) throw new Error('AI 接入需要桌面版，浏览器演示不会创建连接或凭证。');
  return (await import('@tauri-apps/api/core')).invoke<T>(command, args);
}
export const aiApi = {
  async state(): Promise<AiState> { return '__TAURI_INTERNALS__' in window ? invoke('get_ai_state') : { enabled: false, available: false, connections: [], audit: [], limits: { maxRecords: 500, maxResults: 50, maxHours: 168 } }; },
  enable: (enabled: boolean) => invoke<AiState>('set_ai_enabled', { enabled }),
  create: (request: AiRequest) => invoke<{ state: AiState; connectionId: string; config: AiConfig }>('create_ai_connection', { request }),
  revoke: (id: string) => invoke<AiState>('revoke_ai_connection', { id }),
  config: (id: string) => invoke<AiConfig>('get_ai_connection_config', { id }),
  clearAudit: () => invoke<AiState>('clear_ai_audit'),
};
