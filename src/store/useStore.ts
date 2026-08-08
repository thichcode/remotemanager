import { create } from 'zustand';
import type { Server, Group, Credential, Settings, HistoryEntry, SshKey, TerminalTab } from '../types';
import * as api from '../services/tauri';

interface AppState {
  servers: Server[];
  groups: Group[];
  credentials: Credential[];
  settings: Settings | null;
  history: HistoryEntry[];
  sshKeys: SshKey[];
  searchQuery: string;
  selectedGroupId: string | null;
  selectedServerId: string | null;
  isLoading: boolean;
  terminalTabs: TerminalTab[];
  activeTerminalTabId: string | null;

  loadServers: () => Promise<void>;
  loadGroups: () => Promise<void>;
  loadCredentials: () => Promise<void>;
  loadSettings: () => Promise<void>;
  createServer: (server: Omit<Server, 'id' | 'created_at' | 'updated_at'>) => Promise<void>;
  updateServer: (id: string, server: Partial<Server>) => Promise<void>;
  cloneServer: (id: string) => Promise<void>;
  deleteServer: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  searchServers: (query: string) => Promise<void>;
  createGroup: (name: string, parentId?: string | null) => Promise<void>;
  updateGroup: (id: string, name: string) => Promise<void>;
  deleteGroup: (id: string) => Promise<void>;
  createCredential: (name: string, username: string, password: string) => Promise<void>;
  updateCredential: (id: string, name: string, username: string, password?: string) => Promise<void>;
  deleteCredential: (id: string) => Promise<void>;
  updateSettings: (settings: Partial<Settings>) => Promise<void>;
  loadHistory: () => Promise<void>;
  clearHistory: () => Promise<void>;
  loadSshKeys: () => Promise<void>;
  importSshKey: (path: string, name: string, passphrase?: string) => Promise<void>;
  deleteSshKey: (id: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  setSelectedGroup: (id: string | null) => void;
  setSelectedServer: (id: string | null) => void;
  openTerminalTab: (server: Server) => Promise<void>;
  closeTerminalTab: (id: string) => Promise<void>;
  focusTerminalTab: (id: string) => void;
}

export const useStore = create<AppState>((set, get) => ({
  servers: [],
  groups: [],
  credentials: [],
  settings: null,
  history: [],
  sshKeys: [],
  searchQuery: '',
  selectedGroupId: null,
  selectedServerId: null,
  isLoading: false,
  terminalTabs: [],
  activeTerminalTabId: null,

  loadServers: async () => {
    const groupId = get().selectedGroupId;
    if (groupId === '__favorites__') {
      const all = await api.listServers(null);
      set({ servers: all.filter(s => s.favorite) });
      return;
    }
    const servers = await api.listServers(groupId);
    set({ servers });
  },

  loadGroups: async () => {
    const groups = await api.listGroups();
    set({ groups });
  },

  loadCredentials: async () => {
    const credentials = await api.listCredentials();
    set({ credentials });
  },

  loadSettings: async () => {
    const settings = await api.getSettings();
    set({ settings });
  },

  createServer: async (server) => {
    const id = await api.createServer(server as Server);
    const created = { ...(server as Server), id, created_at: new Date().toISOString(), updated_at: new Date().toISOString() } as Server;
    set({ servers: [...get().servers, created] });
  },

  updateServer: async (id, server) => {
    await api.updateServer(id, server);
    set({ servers: get().servers.map(s => s.id === id ? { ...s, ...server, updated_at: new Date().toISOString() } : s) });
  },

  cloneServer: async (id) => {
    const newId = await api.cloneServer(id);
    const src = get().servers.find(s => s.id === id);
    if (src) {
      const cloned = { ...src, id: newId, name: `${src.name} (copy)`, favorite: false, last_connected_at: null };
      set({ servers: [...get().servers, cloned] });
    } else {
      await get().loadServers();
    }
  },

  deleteServer: async (id) => {
    await api.deleteServer(id);
    set({ servers: get().servers.filter(s => s.id !== id) });
  },

  toggleFavorite: async (id) => {
    const wasFavorite = get().servers.find(s => s.id === id)?.favorite ?? false;
    await api.toggleFavorite(id);
    const gid = get().selectedGroupId;
    set({
      servers: gid === '__favorites__' && wasFavorite
        ? get().servers.filter(s => s.id !== id)
        : get().servers.map(s => s.id === id ? { ...s, favorite: !s.favorite } : s),
    });
  },

  searchServers: async (query) => {
    if (!query.trim()) {
      await get().loadServers();
      return;
    }
    const servers = await api.searchServers(query);
    set({ servers });
  },

  createGroup: async (name, parentId) => {
    await api.createGroup(name, parentId);
    await get().loadGroups();
  },

  updateGroup: async (id, name) => {
    await api.updateGroup(id, name);
    await get().loadGroups();
  },

  deleteGroup: async (id) => {
    await api.deleteGroup(id);
    await get().loadGroups();
    await get().loadServers();
  },

  createCredential: async (name, username, password) => {
    await api.createCredential(name, username, password);
    await get().loadCredentials();
  },

  updateCredential: async (id, name, username, password) => {
    await api.updateCredential(id, name, username, password);
    await get().loadCredentials();
  },

  deleteCredential: async (id) => {
    await api.deleteCredential(id);
    await get().loadCredentials();
  },

  updateSettings: async (newSettings) => {
    const current = get().settings;
    if (!current) return;
    const updated = { ...current, ...newSettings };
    await api.updateSettings(updated);
    set({ settings: updated });
  },

  loadHistory: async () => {
    const history = await api.listHistory();
    set({ history });
  },

  clearHistory: async () => {
    await api.clearHistory();
    set({ history: [] });
  },

  loadSshKeys: async () => {
    const sshKeys = await api.listSshKeys();
    set({ sshKeys });
  },

  importSshKey: async (path, name, passphrase) => {
    await api.importSshKey(path, name, passphrase);
    await get().loadSshKeys();
  },

  deleteSshKey: async (id) => {
    await api.deleteSshKey(id);
    await get().loadSshKeys();
  },

  setSearchQuery: (query) => set({ searchQuery: query }),
  setSelectedGroup: (id) => {
    set({ selectedGroupId: id });
    get().loadServers();
  },
  setSelectedServer: (id) => set({ selectedServerId: id }),

  openTerminalTab: async (server) => {
    const tabId = crypto.randomUUID();
    set({
      terminalTabs: [
        ...get().terminalTabs,
        { id: tabId, title: `${server.username || 'user'}@${server.host}`, serverId: server.id, sessionId: null, status: 'connecting' },
      ],
      activeTerminalTabId: tabId,
    });
    try {
      const sessionId = await api.openSshSession({
        host: server.host,
        port: server.port,
        username: server.username,
        serverId: server.id,
        serverName: server.name,
        sshKeyId: server.ssh_key_id,
        credentialId: server.credential_id,
      });
      set({
        terminalTabs: get().terminalTabs.map(t =>
          t.id === tabId ? { ...t, sessionId, status: 'connected' } : t
        ),
      });
    } catch (e) {
      set({
        terminalTabs: get().terminalTabs.map(t =>
          t.id === tabId ? { ...t, status: 'closed' } : t
        ),
      });
      throw e;
    }
  },

  closeTerminalTab: async (id) => {
    const tab = get().terminalTabs.find(t => t.id === id);
    const remaining = get().terminalTabs.filter(t => t.id !== id);
    set({
      terminalTabs: remaining,
      activeTerminalTabId:
        get().activeTerminalTabId === id
          ? remaining.length > 0 ? remaining[0].id : null
          : get().activeTerminalTabId,
    });
    if (tab?.sessionId) {
      try { await api.sshClose(tab.sessionId); } catch { /* already gone */ }
    }
  },

  focusTerminalTab: (id) => set({ activeTerminalTabId: id }),
}));
