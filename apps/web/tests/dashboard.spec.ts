import { test, expect } from '@playwright/test';
const root = process.env.CLICKMAN_TEST_ADMIN_TOKEN ?? 'clickman-ui-test-root-token-32-bytes-long';

test('create a project, issue a source key, ingest events and see reports', async ({ page, request }) => {
  const name = `Browser project ${Date.now()}`;
  await page.goto('./');
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page.getByRole('button', { name: '+ New project' }).click();
  await page.getByLabel('Project name').fill(name);
  await page.getByLabel('Project slug').fill(`browser-${Date.now()}`);
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  const projectId = await page.getByLabel('Project', { exact: true }).inputValue();
  await page.getByRole('button', { name: 'settings', exact: true }).click();
  await page.getByLabel('Source name').fill('browser');
  await page.getByRole('button', { name: 'Create write key' }).click();
  await expect(page.getByLabel('New key')).toHaveValue(/^cmw_/);
  const key = await page.getByLabel('New key').inputValue();
  await page.getByLabel('Processing delay (seconds)').fill('0');
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Project settings saved');
  const at = new Date().toISOString();
  const response = await request.post('v1/batch', { headers: { authorization: `Bearer ${key}` }, data: { sentAt: at, batch: [
    { type: 'track', messageId: crypto.randomUUID(), timestamp: new Date(Date.now() - 1000).toISOString(), event: 'browser_export_started', externalId: 'browser-user', properties: { format: 'mp4' } },
    { type: 'track', messageId: crypto.randomUUID(), timestamp: at, event: 'browser_export', externalId: 'browser-user', properties: { format: 'mp4' } },
  ] } });
  expect(response.status()).toBe(202);
  await page.getByRole('button', { name: 'overview', exact: true }).click();
  await page.getByRole('button', { name: 'Refresh reports', exact: true }).click();
  await expect(page.getByRole('cell', { name: 'browser_export', exact: true })).toBeVisible();
  expect(await page.getByLabel('Project', { exact: true }).inputValue()).toBe(projectId);
  await page.getByRole('button', { name: 'funnels', exact: true }).click();
  await page.getByRole('button', { name: '+ New funnel' }).click();
  await page.getByLabel('Name', { exact: true }).fill('Export conversion');
  await page.getByLabel('Key', { exact: true }).fill('export');
  await page.getByLabel('Event name').nth(0).fill('browser_export_started');
  await page.getByLabel('Event name').nth(1).fill('browser_export');
  await page.getByLabel('Property filters (optional)').nth(1).fill('{"format":"mp4"}');
  await page.getByLabel('Breakdown property (optional)').fill('format');
  await page.getByRole('button', { name: 'Save funnel', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Export conversion', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Refresh reports', exact: true }).click();
  await expect(page.getByRole('cell', { name: 'mp4', exact: true })).toBeVisible();
  await expect(page.getByText('100% of entrants', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'overview', exact: true }).click();
  await page.screenshot({ path: '../../tmp/dashboard-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: '../../tmp/dashboard-mobile.png', fullPage: true });
  await page.getByRole('button', { name: '+ New project' }).click();
  await page.getByLabel('Project name').fill(`Empty ${name}`);
  await page.getByLabel('Project slug').fill(`empty-${Date.now()}`);
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByText('No events yet.', { exact: false })).toBeVisible();
  await expect(page.getByRole('cell', { name: 'browser_export', exact: true })).toHaveCount(0);
  await page.getByLabel('Project', { exact: true }).selectOption(projectId);
  await expect(page.getByRole('cell', { name: 'browser_export', exact: true })).toBeVisible();
});

test('reader sees only their project and signing out clears the session', async ({ page, request }) => {
  async function gql(query: string, variables: Record<string, unknown> = {}) {
    const response = await request.post('graphql', { headers: { authorization: `Bearer ${root}` }, data: { query, variables } });
    const result = await response.json();
    expect(result.errors).toBeUndefined();
    return result.data;
  }
  const name = `Reader project ${Date.now()}`;
  const created = await gql('mutation($slug:String!,$name:String!){createProject(slug:$slug,name:$name){id}}', { slug: `reader-${Date.now()}`, name });
  const projectId = created.createProject.id;
  const issued = await gql('mutation($id:UUID!){issueAccessKey(projectId:$id,name:"Browser reader",role:READER){token}}', { id: projectId });
  await page.goto('./');
  await page.getByLabel('Access token').fill(issued.issueAccessKey.token);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'settings', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: '+ New project' })).toHaveCount(0);
  await expect(page.getByLabel('Project', { exact: true }).locator('option')).toHaveCount(1);
  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page.getByLabel('Access token')).toBeVisible();
  expect(await page.evaluate(() => sessionStorage.getItem('clickman.token'))).toBeNull();
});

test('an invalid access token shows an authentication error', async ({ page }) => {
  await page.goto('./');
  await page.getByLabel('Access token').fill('invalid-test-token');
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(page.getByRole('alert')).toContainText('This access token was rejected');
  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page.getByLabel('Access token')).toBeVisible();
});
