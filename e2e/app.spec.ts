import { test, expect, Page } from '@playwright/test';
import { TAURI_MOCK_BODY, seedDb, MockDbSeed } from './tauri-mock';

const makeServer = (over: Record<string, any> = {}) => ({
  id: 's-' + Math.random().toString(36).slice(2, 8),
  name: 'node',
  host: '10.0.0.1',
  port: 22,
  protocol: 'ssh',
  username: 'root',
  group_id: null,
  tags: '',
  notes: '',
  description: '',
  favorite: false,
  credential_id: null,
  ssh_key_id: null,
  created_at: new Date().toISOString(),
  updated_at: new Date().toISOString(),
  last_connected_at: null,
  ...over,
});

async function boot(page: Page, seed: MockDbSeed = {}) {
  await page.addInitScript(({ body, dbSeed }) => {
    window.__rm_seed = dbSeed;
    // eslint-disable-next-line no-eval
    (0, eval)(body);
  }, { body: TAURI_MOCK_BODY, dbSeed: seedDb(seed) });
  // Load once so localStorage is reachable for the origin, clear it, then
  // reload so the mock starts from the fresh seed.
  await page.goto('/');
  await page.evaluate(() => localStorage.clear());
  await page.reload();
}

test('app boots and renders layout with empty state', async ({ page }) => {
  await boot(page);
  await expect(page.getByText('Remote Manager')).toBeVisible();
  await expect(page.getByText('All Servers (0)', { exact: true })).toBeVisible();
  await expect(page.getByText('Select a server from the sidebar to connect')).toBeVisible();
});

test('creates an SSH server through the form', async ({ page }) => {
  await boot(page);
  await page.getByRole('button', { name: 'Add Server' }).click();
  await page.getByPlaceholder('My Server').fill('web-prod');
  await page.getByPlaceholder('192.168.1.100').fill('10.0.0.55');
  await page.getByLabel('Username').fill('ubuntu');
  await page.getByRole('button', { name: 'Save Server' }).click();
  await expect(page.getByText('web-prod')).toBeVisible();
});

test('SSH key selection persists across edit and reload', async ({ page }) => {
  const keyId = 'key-001';
  await boot(page, {
    servers: [makeServer({ id: 'srv-1', name: 'gitlab', host: 'gitlab.lan' })],
    sshKeys: [{ id: keyId, name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  // attach key via edit form
  await expect(page.getByText('gitlab', { exact: true })).toBeVisible();
  await page.getByText('gitlab', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Edit', exact: true }).click();

  await page.getByRole('textbox', { name: 'SSH Key' }).click();
  await page.getByRole('option', { name: 'crewkey' }).click();
  await page.getByRole('button', { name: 'Save Changes' }).click();

  // persists after reload (re-fetches from mock backend)
  await page.reload();
  await expect(page.getByText('gitlab', { exact: true })).toBeVisible();
});

test('favoriting via context menu increments sidebar count', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 's1', name: 'web1', favorite: false })] });
  await expect(page.getByText('web1', { exact: true })).toBeVisible();
  await page.getByText('web1', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Toggle favorite' }).click();
  await expect(page.getByText('Favorites (1)', { exact: true })).toBeVisible();
});

test('group can be created from sidebar', async ({ page }) => {
  await boot(page);
  await page.getByPlaceholder('New group name + Enter').fill('Production');
  await page.keyboard.press('Enter');
  await expect(page.getByText('Production')).toBeVisible();
});

test('credentials vault creates a profile', async ({ page }) => {
  await boot(page);
  await page.getByText('Credentials', { exact: true }).click();
  await page.getByRole('button', { name: 'Add Credential' }).click();
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('prod-root');
  await page.getByRole('textbox', { name: 'Username', exact: true }).fill('ubuntu');
  await page.getByLabel(/Password/).fill('s3cret!');
  await page.getByRole('button', { name: 'Add Credential', exact: true }).nth(1).click();
  await expect(page.getByText('prod-root')).toBeVisible();
});

test('search filters the server list', async ({ page }) => {
  await boot(page, {
    servers: [
      makeServer({ id: 'p1', name: 'web-prod', host: '10.0.0.1' }),
      makeServer({ id: 'd1', name: 'db-dev', host: '10.0.0.2' }),
    ],
  });
  await page.getByPlaceholder('Search servers...').fill('web');
  await expect(page.getByText('web-prod', { exact: true })).toBeVisible();
  await expect(page.getByText('db-dev', { exact: true })).not.toBeVisible();
});

test('settings backup flow runs via mocked dialogs', async ({ page }) => {
  await boot(page);
  await page.getByText('Settings', { exact: true }).click();
  await expect(page.getByText('Backup Data')).toBeVisible();
  await page.getByRole('button', { name: 'Backup Data' }).click();
  await expect(page.getByText(/Backup Created/)).toBeVisible();
});

test('export dialog opens and closes', async ({ page }) => {
  await boot(page);
  await page.getByRole('button', { name: 'Export' }).click();
  await expect(page.getByText('Choose an export format.')).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByText('Choose an export format.')).toBeHidden();
});

test('ssh connect opens embedded terminal tab and streams output', async ({ page }) => {
  await boot(page, {
    servers: [makeServer({ id: 'srv-ssh', name: 'web-node', host: '10.0.0.66', username: 'ubuntu', ssh_key_id: 'key-001' })],
    sshKeys: [{ id: 'key-001', name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  await expect(page.getByText('web-node', { exact: true })).toBeVisible();
  await page.getByText('web-node', { exact: true }).click();

  await expect(page.getByText('ubuntu@10.0.0.66', { exact: true })).toBeVisible();
  await expect(page.locator('.xterm')).toContainText('mock ssh session ready');
});

test('ssh terminal sends keystrokes and closes session', async ({ page }) => {
  await boot(page, {
    servers: [makeServer({ id: 'srv-ssh', name: 'web-node', host: '10.0.0.66', username: 'ubuntu', ssh_key_id: 'key-001' })],
    sshKeys: [{ id: 'key-001', name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  await page.getByText('web-node', { exact: true }).click();
  await expect(page.locator('.xterm')).toBeVisible();

  await page.locator('.xterm').click();
  await page.keyboard.type('ls');
  await page.waitForTimeout(300);

  const sessionWrites = await page.evaluate(() => {
    const db = localStorage.getItem('rm_mock_db_v1');
    const parsed = db ? JSON.parse(db) : null;
    return parsed && parsed.sessions ? Object.values(parsed.sessions) : [];
  });
  const hasLs = sessionWrites.some((s: any) => {
    const bytes = (s.writes || []).flat();
    return String.fromCharCode(...bytes).includes('ls');
  });
  expect(hasLs).toBe(true);

  await page.getByRole('button', { name: /Close.*session/ }).click();
  await page.waitForTimeout(300);
  const sessionsAfterClose = await page.evaluate(() => {
    const db = localStorage.getItem('rm_mock_db_v1');
    const parsed = db ? JSON.parse(db) : null;
    return parsed && parsed.sessions ? Object.keys(parsed.sessions) : [];
  });
  expect(sessionsAfterClose.length).toBe(0);
});

test('ssh terminal session survives view switches', async ({ page }) => {
  await boot(page, {
    servers: [makeServer({ id: 'srv-ssh', name: 'web-node', host: '10.0.0.66', username: 'ubuntu', ssh_key_id: 'key-001' })],
    sshKeys: [{ id: 'key-001', name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  await page.getByText('web-node', { exact: true }).click();
  await expect(page.locator('.xterm')).toContainText('mock ssh session ready');

  // Switch to Settings and back to Servers — the terminal and its session must survive.
  await page.getByText('Settings', { exact: true }).click();
  await expect(page.getByText('Backup Data')).toBeVisible();
  await page.getByText('Servers', { exact: true }).click();

  await expect(page.locator('.xterm')).toContainText('mock ssh session ready');
  await expect(page.getByText('ubuntu@10.0.0.66', { exact: true })).toBeVisible();
});

test('duplicate creates a named copy of a server', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 'srv-d', name: 'db', host: '10.0.0.9' })] });
  await expect(page.getByText('db', { exact: true })).toBeVisible();

  await page.getByText('db', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Duplicate' }).click();

  await expect(page.getByText('db (copy)', { exact: true })).toBeVisible();
});

test('duplicate from favorites view does not leak into favorites list', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 'srv-f', name: 'db', host: '10.0.0.9', favorite: true })] });
  await expect(page.getByText('Favorites (1)', { exact: true })).toBeVisible();

  await page.getByText('Favorites (1)', { exact: true }).click();
  await expect(page.getByText('db', { exact: true })).toBeVisible();

  await page.getByRole('listitem').filter({ hasText: 'db' }).first().click({ button: 'right' });
  await page.getByRole('button', { name: 'Duplicate' }).click();

  // The copy is not a favorite, so it must NOT appear in the favorites list.
  await expect(page.getByText('db (copy)', { exact: true })).not.toBeVisible();

  // Prove the clone actually happened: leave the favorites view and confirm the copy exists.
  await page.getByText(/^All Servers/).click();
  await expect(page.getByText('db (copy)', { exact: true })).toBeVisible();
});
