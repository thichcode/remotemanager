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
  favorite: boolean;
  credential_id: string | null;
  created_at: string;
  updated_at: string;
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
