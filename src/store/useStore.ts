import { create } from 'zustand';
import type { Server, Group, Credential, Settings, HistoryEntry, SshKey, SessionTab } from '../types';
import * as api from '../services/tauri';

const FAVORITES_ID = '__favorites__';

interface UndoAction {
  type: 'deleteServer';
  data: Server;
  timestamp: number;
}

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
  sessionTabs: SessionTab[];
  activeSessionTabId: string | null;
  expandedGroups: Record<string, boolean>;
  recentServers: Server[];
  undoAction: UndoAction | null;

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
  toggleGroupExpanded: (groupId: string) => void;
  openSession: (server: Server) => Promise<void>;
  openRdpTab: (server: Server) => Promise<void>;
  closeSessionTab: (id: string) => Promise<void>;
  focusSessionTab: (id: string) => void;
  addRecentServer: (server: Server) => void;
  performUndo: () => Promise<void>;
  clearUndo: () => void;
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
  sessionTabs: [],
  activeSessionTabId: null,
  expandedGroups: {},
  recentServers: [],
  undoAction: null,

  loadServers: async () => {
    try {
      set({ isLoading: true });
      const groupId = get().selectedGroupId;
      if (groupId === FAVORITES_ID) {
        const all = await api.listServers(null);
        set({ servers: all.filter(s => s.favorite), isLoading: false });
        return;
      }
      if (groupId === '__recent__') {
        set({ isLoading: false });
        return;
      }
      const servers = await api.listServers(groupId);
      set({ servers, isLoading: false });
    } catch (e: unknown) {
      console.error('Failed to load servers:', e);
      set({ isLoading: false });
    }
  },

  loadGroups: async () => {
    try {
      const groups = await api.listGroups();
      set({ groups });
    } catch (e: unknown) {
      console.error('Failed to load groups:', e);
    }
  },

  loadCredentials: async () => {
    try {
      const credentials = await api.listCredentials();
      set({ credentials });
    } catch (e: unknown) {
      console.error('Failed to load credentials:', e);
    }
  },

  loadSettings: async () => {
    try {
      const settings = await api.getSettings();
      set({ settings });
    } catch (e: unknown) {
      console.error('Failed to load settings:', e);
    }
  },

  createServer: async (server) => {
    const id = await api.createServer(server as Server);
    const created: Server = { ...(server as Server), id, created_at: new Date().toISOString(), updated_at: new Date().toISOString() };
    set({ servers: [...get().servers, created] });
  },

  updateServer: async (id, server) => {
    await api.updateServer(id, server);
    set({ servers: get().servers.map(s => s.id === id ? { ...s, ...server, updated_at: new Date().toISOString() } : s) });
  },

  cloneServer: async (id) => {
    const newId = await api.cloneServer(id);
    const src = get().servers.find(s => s.id === id);
    if (src && get().selectedGroupId !== FAVORITES_ID) {
      const cloned = { ...src, id: newId, name: `${src.name} (copy)`, favorite: false, last_connected_at: null };
      set({ servers: [...get().servers, cloned] });
    } else {
      await get().loadServers();
    }
  },

  deleteServer: async (id) => {
    const server = get().servers.find(s => s.id === id);
    if (!server) return;
    await api.deleteServer(id);
    set({
      servers: get().servers.filter(s => s.id !== id),
      undoAction: { type: 'deleteServer', data: server, timestamp: Date.now() },
    });
  },

  toggleFavorite: async (id) => {
    const wasFavorite = get().servers.find(s => s.id === id)?.favorite ?? false;
    await api.toggleFavorite(id);
    const gid = get().selectedGroupId;
    set({
      servers: gid === FAVORITES_ID && wasFavorite
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
    try {
      const history = await api.listHistory();
      set({ history });
    } catch (e: unknown) {
      console.error('Failed to load history:', e);
    }
  },

  clearHistory: async () => {
    await api.clearHistory();
    set({ history: [] });
  },

  loadSshKeys: async () => {
    try {
      const sshKeys = await api.listSshKeys();
      set({ sshKeys });
    } catch (e: unknown) {
      console.error('Failed to load SSH keys:', e);
    }
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

  toggleGroupExpanded: (groupId) => set((s) => ({
    expandedGroups: { ...s.expandedGroups, [groupId]: !s.expandedGroups[groupId] },
  })),

  openSession: async (server) => {
    const tabId = crypto.randomUUID();
    set({
      sessionTabs: [
        ...get().sessionTabs,
        { id: tabId, title: `${server.username || 'user'}@${server.host}`, protocol: 'ssh', serverId: server.id, sessionId: null, wsPort: null, status: 'connecting' },
      ],
      activeSessionTabId: tabId,
    });
    get().addRecentServer(server);
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
      if (!get().sessionTabs.some(t => t.id === tabId)) {
        try { await api.sshClose(sessionId); } catch {}
        return;
      }
      set({
        sessionTabs: get().sessionTabs.map(t =>
          t.id === tabId ? { ...t, sessionId, status: 'connected' } : t
        ),
      });
    } catch (e) {
      set({
        sessionTabs: get().sessionTabs.map(t =>
          t.id === tabId ? { ...t, status: 'closed' } : t
        ),
      });
      throw e;
    }
  },

  openRdpTab: async (server) => {
    const tabId = crypto.randomUUID();
    set({
      sessionTabs: [
        ...get().sessionTabs,
        { id: tabId, title: server.name, protocol: 'rdp', serverId: server.id, sessionId: null, wsPort: null, status: 'connecting' },
      ],
      activeSessionTabId: tabId,
    });
    get().addRecentServer(server);
    try {
      const wsPort = await api.openRdpSession({
        host: server.host,
        username: server.username,
        fullscreen: get().settings?.rdp_fullscreen ?? false,
        adminMode: get().settings?.rdp_admin_mode ?? false,
        serverId: server.id,
        serverName: server.name,
        credentialId: server.credential_id,
      });
      if (!get().sessionTabs.some(t => t.id === tabId)) {
        try { await api.closeRdpSession(wsPort); } catch {}
        return;
      }
      set({
        sessionTabs: get().sessionTabs.map(t =>
          t.id === tabId ? { ...t, wsPort, status: 'connected' } : t
        ),
      });
    } catch (e) {
      set({
        sessionTabs: get().sessionTabs.map(t =>
          t.id === tabId ? { ...t, status: 'closed' } : t
        ),
      });
      throw e;
    }
  },

  closeSessionTab: async (id) => {
    const tab = get().sessionTabs.find(t => t.id === id);
    const remaining = get().sessionTabs.filter(t => t.id !== id);
    set({
      sessionTabs: remaining,
      activeSessionTabId:
        get().activeSessionTabId === id
          ? remaining.length > 0 ? remaining[0].id : null
          : get().activeSessionTabId,
    });
    if (tab?.protocol === 'ssh' && tab.sessionId) {
      try { await api.sshClose(tab.sessionId); } catch {}
    }
    if (tab?.protocol === 'rdp' && tab.wsPort) {
      try { await api.closeRdpSession(tab.wsPort); } catch {}
    }
  },

  focusSessionTab: (id) => set({ activeSessionTabId: id }),

  addRecentServer: (server) => set((s) => {
    const existing = s.recentServers.filter(r => r.id !== server.id);
    return { recentServers: [server, ...existing].slice(0, 5) };
  }),

  performUndo: async () => {
    const action = get().undoAction;
    if (!action) return;
    if (action.type === 'deleteServer') {
      await api.createServer(action.data);
      const now = new Date().toISOString();
      set({
        servers: [...get().servers, { ...action.data, created_at: now, updated_at: now }],
        undoAction: null,
      });
    }
  },

  clearUndo: () => set({ undoAction: null }),
}));
