import { test, expect, type APIRequestContext } from '@playwright/test';
const root = process.env.CLICKMAN_TEST_ADMIN_TOKEN ?? 'clickman-ui-test-root-token-32-bytes-long';

test('segments preserve paused state when edited and accept a scoped Zapier webhook', async ({
  page,
  request,
}) => {
  const id = await project(request, 'Segment browser contract');
  await page.goto(`projects/${id}/segments`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page.getByRole('button', { name: '+ New segment', exact: true }).click();
  await page.getByLabel('Name', { exact: true }).fill('Checkout interest');
  await page.getByLabel('Key', { exact: true }).fill('interest');
  await page.getByLabel('Event', { exact: true }).fill('checkout_started');
  await page.getByRole('button', { name: 'Save segment', exact: true }).click();
  const row = page.getByRole('row', { name: /Checkout interest/ });
  await expect(row).toContainText('Active');
  await row.getByRole('button', { name: 'Pause', exact: true }).click();
  await expect(row).toContainText('Paused');
  await row.getByRole('button', { name: 'Edit', exact: true }).click();
  await page.getByLabel('Name', { exact: true }).fill('Checkout interest renamed');
  await page.getByRole('button', { name: 'Save segment', exact: true }).click();
  await expect(page.getByRole('row', { name: /Checkout interest renamed/ })).toContainText(
    'Paused',
  );
  await page
    .getByRole('combobox', { name: 'Segment', exact: true })
    .selectOption({ label: 'Checkout interest renamed' });
  await page.getByLabel('Webhook name').fill('Browser Zapier');
  await page.getByRole('combobox', { name: 'Receiver', exact: true }).selectOption('zapier');
  await page
    .getByLabel('Endpoint URL')
    .fill('https://hooks.zapier.com/hooks/catch/fixture/fixture/');
  await page
    .getByLabel('Signing secret environment reference')
    .fill(`CLICKMAN_${id.replaceAll('-', '').toUpperCase()}_SIGNING`);
  await page.getByRole('button', { name: 'Add webhook', exact: true }).click();
  await expect(page.getByText('Browser Zapier · zapier · Active', { exact: true })).toBeVisible();
  const result = await rootGraphql(
    request,
    'query($id:UUID!){project(id:$id){segments{enabled name} webhooks{kind config}}}',
    { id },
  );
  expect(result.project.segments[0].enabled).toBe(false);
  expect(result.project.webhooks[0].kind).toBe('zapier');
});

async function rootGraphql(request: APIRequestContext, query: string, variables = {}) {
  const response = await request.post('graphql', {
    headers: { authorization: `Bearer ${root}` },
    data: { query, variables },
  });
  expect(response.ok()).toBe(true);
  const result = await response.json();
  expect(result.errors).toBeUndefined();
  return result.data;
}

async function project(request: APIRequestContext, name: string): Promise<string> {
  const data = await rootGraphql(
    request,
    'mutation($slug:String!,$name:String!){createProject(slug:$slug,name:$name){id}}',
    { slug: `browser-${crypto.randomUUID()}`, name },
  );
  return data.createProject.id;
}

test('segment edits preserve alternative events and missing-event property filters', async ({
  page,
  request,
}) => {
  const id = await project(request, 'Segment edit contract');
  const definition = {
    key: 'abandoned_paid',
    name: 'Paid checkout without completion',
    rule: {
      kind: 'inactive_after',
      entry: { event: ['checkout_started', 'checkout_resumed'], where: { plan: 'paid' } },
      absent: { event: ['purchase', 'subscription_started'], where: { currency: ['USD', 'EUR'] } },
      after: '24h',
      range: '30d',
    },
  };
  const saved = await rootGraphql(
    request,
    'mutation($id:UUID!,$definition:JSON!){saveSegment(projectId:$id,definition:$definition){definition}}',
    { id, definition },
  );
  await page.goto(`projects/${id}/segments`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page
    .getByRole('row', { name: /Paid checkout without completion/ })
    .getByRole('button', { name: 'Edit', exact: true })
    .click();
  await page.getByLabel('Name', { exact: true }).fill('Renamed paid checkout');
  await page.getByRole('button', { name: 'Save segment', exact: true }).click();
  await expect(page.getByRole('row', { name: /Renamed paid checkout/ })).toBeVisible();
  const result = await rootGraphql(
    request,
    'query($id:UUID!){project(id:$id){segments{definition}}}',
    { id },
  );
  expect(result.project.segments[0].definition.rule).toEqual(saved.saveSegment.definition.rule);
});

test('new segment drafts do not inherit backfill or conditions from an edited segment', async ({
  page,
  request,
}) => {
  const id = await project(request, 'New segment draft contract');
  await rootGraphql(
    request,
    'mutation($id:UUID!,$definition:JSON!){saveSegment(projectId:$id,definition:$definition,fireExisting:true){id}}',
    {
      id,
      definition: {
        key: 'existing',
        name: 'Existing backfill audience',
        rule: {
          kind: 'event',
          filter: { event: 'purchase', where: { plan: 'paid' } },
          range: '30d',
        },
      },
    },
  );
  await page.goto(`projects/${id}/segments`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page
    .getByRole('row', { name: /Existing backfill audience/ })
    .getByRole('button', { name: 'Edit', exact: true })
    .click();
  await expect(page.getByLabel('Emit entry events', { exact: false })).toBeChecked();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: '+ New segment', exact: true }).click();
  await expect(page.getByLabel('Emit entry events', { exact: false })).not.toBeChecked();
  await expect(page.getByLabel('Event', { exact: true })).toHaveValue('');
  await expect(page.getByLabel('Property filters (JSON, optional)', { exact: true })).toHaveValue(
    '',
  );
});

test('funnel editing preserves alternatives, literal event names and property filters', async ({
  page,
  request,
}) => {
  const id = await project(request, 'Funnel editor contract');
  const definition = {
    key: 'purchase',
    name: 'Purchase before edit',
    window: '24h',
    range: '30d',
    breakdown: 'plan',
    steps: [
      { event: ['checkout_started', 'checkout_resumed'], where: { plan: ['paid', 'trial'] } },
      { event: ' purchase,completed ', where: { currency: 'USD' } },
    ],
  };
  const saved = await rootGraphql(
    request,
    'mutation($id:UUID!,$definition:JSON!){saveFunnel(projectId:$id,definition:$definition){definition}}',
    { id, definition },
  );
  await page.goto(`projects/${id}/funnels`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page
    .getByRole('region', { name: 'Purchase before edit', exact: true })
    .getByRole('button', { name: 'Edit', exact: true })
    .click();
  await page.getByLabel('Name', { exact: true }).fill('Purchase after edit');
  await page.getByRole('button', { name: 'Save funnel', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Purchase after edit', exact: true }),
  ).toBeVisible();
  const result = await rootGraphql(
    request,
    'query($id:UUID!){project(id:$id){funnels{definition}}}',
    { id },
  );
  expect(result.project.funnels[0].definition).toEqual({
    ...saved.saveFunnel.definition,
    name: 'Purchase after edit',
  });
});

test.beforeEach(async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  (page as typeof page & { runtimeErrors: string[] }).runtimeErrors = errors;
});

test.afterEach(async ({ page }) => {
  expect((page as typeof page & { runtimeErrors: string[] }).runtimeErrors).toEqual([]);
});

test('create a project, issue a source key, ingest events and see reports', async ({
  page,
  request,
}) => {
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
  const response = await request.post('v1/batch', {
    headers: { authorization: `Bearer ${key}` },
    data: {
      sentAt: at,
      batch: [
        {
          type: 'track',
          messageId: crypto.randomUUID(),
          timestamp: new Date(Date.now() - 1000).toISOString(),
          event: 'browser_export_started',
          externalId: 'browser-user',
          properties: { format: 'mp4' },
        },
        {
          type: 'track',
          messageId: crypto.randomUUID(),
          timestamp: at,
          event: 'browser_export',
          externalId: 'browser-user',
          properties: { format: 'mp4' },
        },
      ],
    },
  });
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
  await expect(page.getByRole('button', { name: 'overview', exact: true })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await page.screenshot({ path: '../../tmp/dashboard-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
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

test('reader sees only their project and signing out clears the session', async ({
  page,
  request,
}) => {
  async function gql(query: string, variables: Record<string, unknown> = {}) {
    const response = await request.post('graphql', {
      headers: { authorization: `Bearer ${root}` },
      data: { query, variables },
    });
    const result = await response.json();
    expect(result.errors).toBeUndefined();
    return result.data;
  }
  const name = `Reader project ${Date.now()}`;
  const created = await gql(
    'mutation($slug:String!,$name:String!){createProject(slug:$slug,name:$name){id}}',
    { slug: `reader-${Date.now()}`, name },
  );
  const projectId = created.createProject.id;
  const issued = await gql(
    'mutation($id:UUID!){issueAccessKey(projectId:$id,name:"Browser reader",role:READER){token}}',
    { id: projectId },
  );
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

test('protected project deep links survive login, reload and browser history', async ({
  page,
  request,
}) => {
  const response = await request.post('graphql', {
    headers: { authorization: `Bearer ${root}` },
    data: {
      query: 'mutation($slug:String!,$name:String!){createProject(slug:$slug,name:$name){id}}',
      variables: { slug: `deep-${Date.now()}`, name: 'Deep-linked project' },
    },
  });
  const result = await response.json();
  expect(result.errors).toBeUndefined();
  const id = result.data.createProject.id;
  await page.goto(`projects/${id}/retention`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(
    page.getByRole('heading', { name: 'Deep-linked project', exact: true }),
  ).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Cohort retention', exact: true })).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/projects/${id}/retention$`));
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Cohort retention', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'overview', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${id}/overview$`));
  await page.goBack();
  await expect(page.getByRole('heading', { name: 'Cohort retention', exact: true })).toBeVisible();
  await page.getByRole('link', { name: /clickman$/ }).click();
  await expect(page).toHaveURL(/\/projects\/[0-9a-f-]+\/overview$/);
  await page.goBack();
  await expect(page.getByRole('heading', { name: 'Cohort retention', exact: true })).toBeVisible();
});

test('key issuance locks its draft until the updated key list is loaded', async ({
  page,
  request,
}) => {
  const response = await request.post('graphql', {
    headers: { authorization: `Bearer ${root}` },
    data: {
      query: 'mutation($slug:String!,$name:String!){createProject(slug:$slug,name:$name){id}}',
      variables: { slug: `keys-${Date.now()}`, name: 'Key draft project' },
    },
  });
  const result = await response.json();
  expect(result.errors).toBeUndefined();
  const id = result.data.createProject.id;
  await page.goto(`projects/${id}/settings`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page.getByLabel('Source name').fill('frozen-source');
  let releaseMutation!: () => void;
  let capturedMutation!: () => void;
  let releaseRefresh!: () => void;
  let capturedRefresh!: () => void;
  const mutationHeld = new Promise<void>((resolve) => {
    releaseMutation = resolve;
  });
  const mutationReady = new Promise<void>((resolve) => {
    capturedMutation = resolve;
  });
  const refreshHeld = new Promise<void>((resolve) => {
    releaseRefresh = resolve;
  });
  const refreshReady = new Promise<void>((resolve) => {
    capturedRefresh = resolve;
  });
  let refreshArmed = false;
  let refreshCaptured = false;
  await page.route('**/graphql', async (route) => {
    const body = route.request().postDataJSON();
    if (body.operationName === 'IssueWriteKey') {
      const response = await route.fetch();
      capturedMutation();
      await mutationHeld;
      await route.fulfill({ response });
    } else if (body.operationName === 'ProjectAdmin' && refreshArmed && !refreshCaptured) {
      refreshCaptured = true;
      const response = await route.fetch();
      capturedRefresh();
      await refreshHeld;
      await route.fulfill({ response });
    } else await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Create write key', exact: true }).click();
    await mutationReady;
    await expect(page.getByLabel('Source name')).toBeDisabled();
    refreshArmed = true;
    releaseMutation();
    await refreshReady;
    await expect(page.getByLabel('New key')).toHaveValue(/^cmw_/);
    await expect(page.getByLabel('Source name')).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Create write key', exact: true }),
    ).toBeDisabled();
    releaseRefresh();
    await expect(page.getByLabel('Source name')).toBeEnabled();
  } finally {
    releaseMutation();
    releaseRefresh();
    await page.unrouteAll({ behavior: 'wait' });
  }
});

test('a project UUID in uppercase opens the same analytics report', async ({ page, request }) => {
  const id = await project(request, 'Uppercase project link');
  await page.goto(`projects/${id.toUpperCase()}/retention`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(
    page.getByRole('heading', { name: 'Uppercase project link', exact: true }),
  ).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Cohort retention', exact: true })).toBeVisible();
});

test('funnel drafts stay locked and follow-up refresh failures are visible', async ({
  page,
  request,
}) => {
  const id = await project(request, 'Funnel draft project');
  await page.goto(`projects/${id}/funnels`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await page.getByRole('button', { name: '+ New funnel' }).click();
  await page.getByLabel('Name', { exact: true }).fill('Locked funnel');
  await page.getByLabel('Key', { exact: true }).fill('locked_funnel');
  await page.getByLabel('Event name').nth(0).fill('started');
  await page.getByLabel('Event name').nth(1).fill('finished');
  let release!: () => void;
  let captured!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const ready = new Promise<void>((resolve) => {
    captured = resolve;
  });
  let refreshArmed = false;
  await page.route('**/graphql', async (route) => {
    const body = route.request().postDataJSON();
    if (body.operationName === 'SaveFunnel') {
      const response = await route.fetch();
      refreshArmed = true;
      captured();
      await held;
      await route.fulfill({ response });
    } else if (body.operationName === 'Analytics' && refreshArmed) {
      await route.fulfill({ json: { errors: [{ message: 'Test report refresh unavailable' }] } });
    } else await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Save funnel', exact: true }).click();
    await ready;
    await expect(page.getByLabel('Name', { exact: true })).toBeDisabled();
    await expect(page.getByRole('button', { name: '+ New funnel' })).toBeDisabled();
    release();
    await expect(page.getByRole('alert').first()).toContainText('Test report refresh unavailable');
    await expect(page.getByRole('button', { name: '+ New funnel' })).toBeEnabled();
  } finally {
    release();
    await page.unrouteAll({ behavior: 'wait' });
  }
});

test('switching projects hides cached data and ignores a late previous-project response', async ({
  page,
  request,
}) => {
  const oldId = await project(request, 'Previous project');
  const newId = await project(request, 'Current empty project');
  await rootGraphql(
    request,
    'mutation($id:UUID!,$definition:JSON!){saveFunnel(projectId:$id,definition:$definition){id}}',
    {
      id: oldId,
      definition: {
        key: 'old',
        name: 'Previous project funnel',
        window: '7d',
        range: '90d',
        steps: [{ event: 'first' }, { event: 'last' }],
      },
    },
  );
  await page.goto(`projects/${oldId}/funnels`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(
    page.getByRole('heading', { name: 'Previous project funnel', exact: true }),
  ).toBeVisible();
  let release!: () => void;
  let captured!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const ready = new Promise<void>((resolve) => {
    captured = resolve;
  });
  let capturedOnce = false;
  await page.route('**/graphql', async (route) => {
    const body = route.request().postDataJSON();
    if (body.operationName === 'Analytics' && body.variables.id === oldId && !capturedOnce) {
      capturedOnce = true;
      const response = await route.fetch();
      captured();
      await held;
      await route.fulfill({ response });
    } else await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Refresh reports', exact: true }).click();
    await ready;
    await page.getByLabel('Project', { exact: true }).selectOption(newId);
    await expect(
      page.getByRole('heading', { name: 'Current empty project', exact: true }),
    ).toBeVisible();
    await expect(page.getByText('No funnels yet.', { exact: false })).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Previous project funnel', exact: true }),
    ).toHaveCount(0);
    release();
    await page.unrouteAll({ behavior: 'wait' });
    await expect(page.getByLabel('Project', { exact: true })).toHaveValue(newId);
    await expect(
      page.getByRole('heading', { name: 'Previous project funnel', exact: true }),
    ).toHaveCount(0);
  } finally {
    release();
    await page.unrouteAll({ behavior: 'wait' });
  }
});

test('a late administrator response cannot enter a new reader session', async ({
  page,
  request,
}) => {
  const oldId = await project(request, 'Old session project');
  const readerId = await project(request, 'New reader session project');
  const data = await rootGraphql(
    request,
    'mutation($id:UUID!){issueAccessKey(projectId:$id,name:"Session reader",role:READER){token}}',
    { id: readerId },
  );
  await page.goto(`projects/${oldId}/overview`);
  await page.getByLabel('Access token').fill(root);
  await page.getByRole('button', { name: 'Open dashboard' }).click();
  await expect(
    page.getByRole('heading', { name: 'Old session project', exact: true }),
  ).toBeVisible();
  let release!: () => void;
  let captured!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const ready = new Promise<void>((resolve) => {
    captured = resolve;
  });
  let capturedOnce = false;
  await page.route('**/graphql', async (route) => {
    const body = route.request().postDataJSON();
    if (body.operationName === 'Analytics' && body.variables.id === oldId && !capturedOnce) {
      capturedOnce = true;
      const response = await route.fetch();
      captured();
      await held;
      await route.fulfill({ response });
    } else await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Refresh reports', exact: true }).click();
    await ready;
    await page.getByRole('button', { name: 'Sign out' }).click();
    await page.getByLabel('Access token').fill(data.issueAccessKey.token);
    await page.getByRole('button', { name: 'Open dashboard' }).click();
    await expect(
      page.getByRole('heading', { name: 'New reader session project', exact: true }),
    ).toBeVisible();
    release();
    await page.unrouteAll({ behavior: 'wait' });
    await expect(page.getByLabel('Project', { exact: true }).locator('option')).toHaveCount(1);
    await expect(
      page.getByRole('heading', { name: 'Old session project', exact: true }),
    ).toHaveCount(0);
    await page.goto(`projects/${readerId}/settings`);
    await expect(
      page.getByRole('heading', { name: 'Project administrator access required.', exact: true }),
    ).toBeVisible();
    await expect(page.getByLabel('Source name')).toHaveCount(0);
  } finally {
    release();
    await page.unrouteAll({ behavior: 'wait' });
  }
});
