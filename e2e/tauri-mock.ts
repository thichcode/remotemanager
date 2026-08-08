// Mocked of the Rust backend Tauri is injected as window.__TAURI_INTERNALS__
// before the UI boots. Kept fully self-contained (no external imports) because
// Playwright serializes addInitScript functions into the browser context.

export const TAURI_MOCK_BODY = String.raw`(() => {
  const STORE_KEY = 'rm_mock_db_v1';
  let db = null;
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (saved) db = JSON.parse(saved);
  } catch (e) { /* ignore */ }
  if (!db) {
    db = (window.__rm_seed && window.__rm_seed.db) || {
      servers: [],
      groups: [],
      credentials: [],
      sshKeys: [],
      settings: { id: 1, theme: 'dark', font_size: 14, ssh_port: 22, rdp_fullscreen: false, rdp_admin_mode: false },
    };
    try { localStorage.setItem(STORE_KEY, JSON.stringify(db)); } catch (e) { /* ignore */ }
  }
  const save = () => { try { localStorage.setItem(STORE_KEY, JSON.stringify(db)); } catch (e) { /* ignore */ } };
  const uid = () => (Math.random().toString(36) + Date.now().toString(36)).slice(2, 14);

  const listeners = {};
  let eventIdCounter = 1;
  const emit = (event, payload) => {
    const byId = listeners[event] || {};
    Object.keys(byId).forEach((id) => byId[id]({ event, id: Number(id), payload }));
  };
  window.__rm_emit = emit;
  window.__rm_listeners = listeners;

  const serverFromArgs = (a) => {
    const now = new Date().toISOString();
    return {
      id: a.id || uid(),
      name: a.name || '',
      host: a.host || '',
      port: Number(a.port) || (a.protocol === 'rdp' ? 3389 : 22),
      protocol: a.protocol || 'ssh',
      username: a.username || '',
      group_id: a.group_id || null,
      tags: a.tags || '',
      notes: a.notes || '',
      description: a.description || '',
      favorite: !!a.favorite,
      credential_id: a.credential_id || null,
      ssh_key_id: a.ssh_key_id || null,
      created_at: now,
      updated_at: now,
      last_connected_at: null,
    };
  };

  const handler = {
    cmd_create_server: (a) => { const s = serverFromArgs(a); s.id = uid(); db.servers.push(s); return s.id; },
    cmd_update_server: (a) => {
      const s = db.servers.find((x) => x.id === a.id);
      if (!s) throw new Error('server not found');
      const patch = serverFromArgs(a);
      Object.assign(s, patch, { id: a.id, updated_at: new Date().toISOString() });
      return null;
    },
    cmd_delete_server: (a) => { db.servers = db.servers.filter((x) => x.id !== a.id); return null; },
    cmd_get_server: (a) => db.servers.find((x) => x.id === a.id) || null,
    cmd_list_servers: () => db.servers.map((s) => ({ ...s })),
    cmd_toggle_favorite: (a) => {
      const s = db.servers.find((x) => x.id === a.id);
      if (!s) throw new Error('server not found');
      s.favorite = !s.favorite;
      s.updated_at = new Date().toISOString();
      return s.favorite;
    },
    cmd_search_servers: (a) => {
      const q = ((a && a.query) || '').toLowerCase();
      return db.servers.filter((s) => (s.name || '').toLowerCase().includes(q) || (s.host || '').toLowerCase().includes(q));
    },
    cmd_clone_server: (a) => {
      const s = db.servers.find((x) => x.id === a.id);
      if (!s) throw new Error('server not found');
      const copy = { ...s, id: uid(), name: s.name + ' (copy)', favorite: false };
      db.servers.push(copy);
      return copy.id;
    },
    cmd_list_groups: () => db.groups.slice(),
    cmd_create_group: (a) => { const g = { id: uid(), name: a.name, parent_id: a.parent_id || null, sort_order: db.groups.length }; db.groups.push(g); return g.id; },
    cmd_update_group: (a) => { const g = db.groups.find((x) => x.id === a.id); if (g) g.name = a.name; return null; },
    cmd_delete_group: (a) => { db.groups = db.groups.filter((x) => x.id !== a.id); return null; },
    cmd_list_credentials: () => db.credentials.map(({ password, ...c }) => c),
    cmd_create_credential: (a) => { const c = { id: uid(), name: a.name, username: a.username, password: a.password || '', created_at: new Date().toISOString() }; db.credentials.push(c); return c.id; },
    cmd_update_credential: () => null,
    cmd_delete_credential: (a) => { db.credentials = db.credentials.filter((x) => x.id !== a.id); return null; },
    cmd_test_credential: () => 'OK',
    cmd_get_settings: () => ({ ...db.settings }),
    cmd_update_settings: (a) => { Object.assign(db.settings, a); return null; },
    cmd_is_portable: () => false,
    cmd_list_history: () => [],
    cmd_clear_history: () => null,
    cmd_list_ssh_keys: () => db.sshKeys.map(({ path, ...rest }) => ({ ...rest })),
    cmd_import_ssh_key: (a) => { const k = { id: uid(), name: a.name || 'key', public_key: 'ssh-ed25519 AAAA' + uid(), created_at: new Date().toISOString() }; db.sshKeys.push(k); return k.id; },
    cmd_delete_ssh_key: (a) => { db.sshKeys = db.sshKeys.filter((x) => x.id !== a.id); return null; },
    cmd_attach_key: (a) => {
      const s = db.servers.find((x) => x.id === a.server_id);
      if (!s) throw new Error('server not found');
      s.ssh_key_id = a.ssh_key_id || null;
      return null;
    },
    cmd_list_tags: () => [...new Set(db.servers.flatMap((s) => (s.tags || '').split(',').map((t) => t.trim()).filter(Boolean)))],
    cmd_set_server_tags: (a) => { const s = db.servers.find((x) => x.id === a.server_id); if (s) s.tags = (a.tags || []).join(','); return null; },
    cmd_list_tags_for_server: (a) => { const s = db.servers.find((x) => x.id === a.server_id); return (s && s.tags ? s.tags.split(',').map((t) => t.trim()).filter(Boolean) : []); },
    cmd_list_recent_servers: () => db.servers.slice(),
    cmd_launch_ssh: () => null,
    cmd_launch_rdp: () => null,
    cmd_open_ssh_session: (a) => {
      const sid = 'sess-' + uid();
      db.sessions = db.sessions || {};
      db.sessions[sid] = { server_id: a.server_id, writes: [] };
      setTimeout(() => {
        emit('ssh://output', { sessionId: sid, data: Array.from(new TextEncoder().encode('mock ssh session ready\r\n')) });
      }, 50);
      return sid;
    },
    cmd_ssh_write: (a) => {
      const s = db.sessions && db.sessions[a.session_id];
      if (s) {
        s.writes.push(a.data);
        emit('ssh://output', { sessionId: a.session_id, data: a.data });
      }
      return null;
    },
    cmd_ssh_resize: () => null,
    cmd_ssh_close: (a) => { if (db.sessions) delete db.sessions[a.session_id]; return null; },
    cmd_ssh_close_all: () => { db.sessions = {}; return null; },
    cmd_ping: () => 'Reachable: 0ms',
    cmd_import_csv: () => ({ imported: 0, errors: [] }),
    cmd_import_json: () => ({ imported: 0, errors: [] }),
    cmd_backup: (a) => ({ file: a.path, db_size: 12345, keys_count: db.sshKeys.length }),
    cmd_restore: () => '/mock/safety-dir',
  };

  let callbackId = 1;
  const callbacks = new Map();
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (event, eventId) => {
      if (listeners[event]) delete listeners[event][eventId];
    },
  };
  window.__TAURI_INTERNALS__ = {
    invoke: (cmd, args) => {
      if (cmd === 'plugin:event|listen') {
        const { event, handler } = args || {};
        const eventId = eventIdCounter++;
        listeners[event] = listeners[event] || {};
        listeners[event][eventId] = (e) => window.__TAURI_INTERNALS__.runCallback(handler, e);
        return Promise.resolve(eventId);
      }
      if (cmd === 'plugin:event|unlisten') return Promise.resolve(null);
      if (cmd.startsWith('plugin:')) {
        if (cmd.includes('dialog')) return Promise.resolve('/mock/path');
        return Promise.resolve(null);
      }
      if (typeof handler[cmd] !== 'function') return Promise.reject('unknown command: ' + cmd);
      try {
        const result = handler[cmd](args || {});
        save();
        return Promise.resolve(result);
      } catch (e) {
        return Promise.reject((e && e.message) ? e.message : String(e));
      }
    },
    transformCallback: (cb) => { const t = callbackId++; callbacks.set(t, cb); return t; },
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (t, ...a) => { const cb = callbacks.get(t); if (cb) cb(...a); },
    callbacks,
    convertFileSrc: (p) => p,
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
  };
})();`;

export interface MockDbSeed {
  servers?: any[];
  groups?: any[];
  credentials?: any[];
  sshKeys?: any[];
  settings?: any;
  sessions?: any;
}

export function seedDb(seed: MockDbSeed = {}) {
  return {
    db: {
      servers: seed.servers || [],
      groups: seed.groups || [],
      credentials: seed.credentials || [],
      sshKeys: seed.sshKeys || [],
      settings: seed.settings || { id: 1, theme: 'dark', font_size: 14, ssh_port: 22, rdp_fullscreen: false, rdp_admin_mode: false },
      sessions: seed.sessions || {},
    },
  };
}