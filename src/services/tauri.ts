import { invoke } from '@tauri-apps/api/core';
import type { Server, Group, Credential, Settings } from '../types';

// Servers
export const createServer = (server: Server): Promise<string> =>
  invoke('cmd_create_server', { ...server });

export const updateServer = (id: string, server: Partial<Server>): Promise<void> =>
  invoke('cmd_update_server', { id, ...server });

export const deleteServer = (id: string): Promise<void> =>
  invoke('cmd_delete_server', { id });

export const getServer = (id: string): Promise<Server | null> =>
  invoke('cmd_get_server', { id });

export const listServers = (groupId?: string | null): Promise<Server[]> =>
  invoke('cmd_list_servers', { groupId });

export const toggleFavorite = (id: string): Promise<boolean> =>
  invoke('cmd_toggle_favorite', { id });

export const searchServers = (query: string): Promise<Server[]> =>
  invoke('cmd_search_servers', { query });

// Groups
export const createGroup = (name: string, parentId?: string | null): Promise<string> =>
  invoke('cmd_create_group', { name, parentId });

export const updateGroup = (id: string, name: string): Promise<void> =>
  invoke('cmd_update_group', { id, name });

export const deleteGroup = (id: string): Promise<void> =>
  invoke('cmd_delete_group', { id });

export const listGroups = (): Promise<Group[]> =>
  invoke('cmd_list_groups');

// SSH / RDP
export const launchSsh = (host: string, port: number, username: string): Promise<void> =>
  invoke('cmd_launch_ssh', { host, port, username });

export const launchRdp = (host: string, username: string, fullscreen: boolean, adminMode: boolean): Promise<void> =>
  invoke('cmd_launch_rdp', { host, username, fullscreen, adminMode });

// Ping
export const pingHost = (host: string): Promise<string> =>
  invoke('cmd_ping', { host });

// Credentials
export const createCredential = (name: string, username: string, password: string): Promise<string> =>
  invoke('cmd_create_credential', { name, username, password });

export const deleteCredential = (id: string): Promise<void> =>
  invoke('cmd_delete_credential', { id });

export const listCredentials = (): Promise<Credential[]> =>
  invoke('cmd_list_credentials');

export const getCredentialPassword = (id: string): Promise<string> =>
  invoke('cmd_get_credential_password', { id });

// Import/Export
export const importCsv = (path: string): Promise<{ imported: number; errors: string[] }> =>
  invoke('cmd_import_csv', { path });

export const exportCsv = (path: string): Promise<void> =>
  invoke('cmd_export_csv', { path });

export const exportJson = (path: string): Promise<void> =>
  invoke('cmd_export_json', { path });

export const importJson = (path: string): Promise<{ imported: number; errors: string[] }> =>
  invoke('cmd_import_json', { path });

// Settings
export const getSettings = (): Promise<Settings> =>
  invoke('cmd_get_settings');

export const updateSettings = (settings: Settings): Promise<void> =>
  invoke('cmd_update_settings', { ...settings });
