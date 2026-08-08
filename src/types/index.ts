export type Protocol = 'ssh' | 'rdp';

export interface Server {
  id: string;
  name: string;
  host: string;
  port: number;
  protocol: Protocol;
  username: string;
  group_id: string | null;
  tags: string;
  notes: string;
  description: string;
  favorite: boolean;
  credential_id: string | null;
  ssh_key_id: string | null;
  created_at: string;
  updated_at: string;
  last_connected_at: string | null;
}

export interface Group {
  id: string;
  name: string;
  parent_id: string | null;
  sort_order: number;
}

export interface Credential {
  id: string;
  name: string;
  username: string;
  created_at: string;
}

export interface Settings {
  id: number;
  theme: 'light' | 'dark';
  font_size: number;
  ssh_port: number;
  rdp_fullscreen: boolean;
  rdp_admin_mode: boolean;
}

export interface ImportResult {
  imported: number;
  errors: string[];
}

export interface HistoryEntry {
  id: string;
  server_id: string | null;
  server_name: string;
  host: string;
  port: number | null;
  protocol: Protocol;
  username: string;
  ssh_key_id: string | null;
  connected_at: string;
  status: string;
}

export interface SshKey {
  id: string;
  name: string;
  public_key: string;
  created_at: string;
}

export interface BackupSummary {
  file: string;
  db_size: number;
  keys_count: number;
}

export interface TerminalTab {
  id: string;
  title: string;
  serverId: string | null;
  sessionId: string | null;
  status: 'connecting' | 'connected' | 'closed';
}
