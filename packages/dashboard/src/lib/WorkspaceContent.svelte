<script lang="ts">
  import { untrack } from 'svelte';
  import { createQuery, createMutation } from 'cachebay/svelte';
  import { Button } from './ui/index';
  import {
    ProjectsDocument,
    DashboardAccessDocument,
    AnalyticsDocument,
    RefreshAnalyticsDocument,
    type ProjectsQuery,
    type DashboardAccessQuery,
    type AnalyticsQuery,
    type AnalyticsQueryVariables,
    type RefreshAnalyticsMutation,
    type RefreshAnalyticsMutationVariables,
  } from './graphql/generated';
  import { dataOrThrow, errorMessage } from './graphql/errors';
  import { pollVisible } from './graphql/poll.svelte';
  import type { ClickmanRoute, ClickmanWorkspaceProps } from './types';
  import SegmentsScreen from './segments/SegmentsScreen.svelte';
  import type { ActiveDay, Cohort, EventTotal, FunnelReport, Settings } from './reports';
  import ProjectPicker from './projects/ProjectPicker.svelte';
  import OverviewScreen from './overview/OverviewScreen.svelte';
  import RetentionScreen from './retention/RetentionScreen.svelte';
  import FunnelsScreen from './funnels/FunnelsScreen.svelte';
  import SettingsScreen from './settings/SettingsScreen.svelte';

  let {
    route,
    onnavigate,
    createWebhook,
    createDestination,
    allowAccessKeys,
    maxRetentionDays,
  }: Omit<ClickmanWorkspaceProps, 'client'> = $props();
  const projectsQuery = createQuery<ProjectsQuery>({ query: ProjectsDocument });
  // Permissions can arrive while a slow project list is still loading.
  const accessQuery = createQuery<DashboardAccessQuery>({ query: DashboardAccessDocument });
  const analytics = createQuery<AnalyticsQuery, AnalyticsQueryVariables>({
    query: AnalyticsDocument,
    variables: () => ({ id: route.projectId ?? '' }),
    enabled: () => !!route.projectId,
  });
  const refresh = createMutation<RefreshAnalyticsMutation, RefreshAnalyticsMutationVariables>({
    query: RefreshAnalyticsDocument,
  });
  let notice = $state('');
  let creating = $state(false);
  let refreshing = $state(false);
  const projects = $derived(projectsQuery.data?.projects.edges.map((edge) => edge.node) ?? []);
  // Cachebay can keep the previous response while fetching new variables.
  const project = $derived(
    analytics.data?.project.id === route.projectId ? analytics.data.project : undefined,
  );
  const current = $derived(project ?? projects.find((item) => item.id === route.projectId));
  const reports = $derived(project?.reports ?? []);
  const days = $derived(
    (
      reports.find((report) => report.key === 'actives')?.result as
        { days?: ActiveDay[] } | undefined
    )?.days ?? [],
  );
  const cohorts = $derived(
    (
      reports.find((report) => report.key === 'retention')?.result as
        { cohorts?: Cohort[] } | undefined
    )?.cohorts ?? [],
  );
  const events = $derived(
    (
      reports.find((report) => report.key === 'events')?.result as
        { events?: EventTotal[] } | undefined
    )?.events ?? [],
  );
  const funnels = $derived(
    reports
      .filter((report) => report.key.startsWith('funnel:'))
      .map((report) => report.result as FunnelReport),
  );
  const computed = $derived(reports[0]?.computedAt);

  async function navigate(next: ClickmanRoute) {
    notice = '';
    try {
      await onnavigate(next);
    } catch (cause) {
      notice = errorMessage(cause);
    }
  }
  $effect(() => {
    if (!route.projectId && !creating && projects.length) {
      const next = { projectId: projects[0].id, section: route.section };
      untrack(() => {
        void navigate(next);
      });
    }
  });
  pollVisible(
    analytics,
    () => !!route.projectId,
    (cause) => {
      notice = errorMessage(cause);
    },
  );

  async function created(id: string) {
    creating = true;
    try {
      await projectsQuery.refetch();
      if (projectsQuery.error) throw projectsQuery.error;
      await onnavigate({ projectId: id, section: 'overview' });
    } finally {
      creating = false;
    }
  }
  async function reloadAnalytics() {
    await analytics.refetch();
    if (analytics.error) throw analytics.error;
  }
  async function updateReports() {
    if (!route.projectId || refreshing) return;
    refreshing = true;
    notice = '';
    try {
      dataOrThrow(await refresh.execute({ projectId: route.projectId }));
      await reloadAnalytics();
    } catch (cause) {
      notice = errorMessage(cause);
    } finally {
      refreshing = false;
    }
  }
</script>

{#if projectsQuery.error}<div role="alert" class="cm-error cm:mb-5">
    {errorMessage(projectsQuery.error)}
  </div>{/if}
{#if accessQuery.error && (!projectsQuery.error || errorMessage(accessQuery.error) !== errorMessage(projectsQuery.error))}<div
    role="alert"
    class="cm-error cm:mb-5"
  >
    {errorMessage(accessQuery.error)}
  </div>{/if}
{#if notice}<div role="alert" class="cm-error cm:mb-5">{notice}</div>{/if}
<ProjectPicker
  {projects}
  selected={route.projectId}
  canCreate={accessQuery.data?.viewer.canCreateProjects ?? false}
  onselect={(id) => {
    void navigate({ projectId: id, section: route.section });
  }}
  oncreated={created}
/>

{#if route.projectId}
  <div class="cm:flex cm:flex-wrap cm:items-center cm:justify-between cm:gap-5">
    <div>
      <p class="cm-section-label cm:mb-2">{current?.slug ?? ''}</p>
      <h1>{current?.name ?? 'Project'}</h1>
      <p class="cm:mt-2 cm:text-sm cm:text-muted">Activity, retention and the paths people take.</p>
    </div>
    <div class="cm:flex cm:items-center cm:gap-4">
      <span class="cm:text-xs cm:text-muted"
        >{computed
          ? `Updated ${new Date(computed).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
          : 'Waiting for first report'}</span
      >{#if current?.canAdmin}<Button
          variant="secondary"
          disabled={refreshing}
          onclick={updateReports}>{refreshing ? 'Updating…' : 'Refresh reports'}</Button
        >{/if}
    </div>
  </div>
  <nav
    aria-label="Analytics sections"
    class="cm:mb-7 cm:mt-7 cm:flex cm:flex-wrap cm:gap-7 cm:border-b cm:border-line"
  >
    {#each ['overview', 'retention', 'funnels', 'segments', ...(current?.canAdmin ? ['settings'] : [])] as section}
      <button
        type="button"
        class="cm-tab cm:capitalize"
        class:active={route.section === section}
        aria-current={route.section === section ? 'page' : undefined}
        onclick={() => {
          void navigate({
            projectId: route.projectId,
            section: section as ClickmanRoute['section'],
          });
        }}>{section}</button
      >
    {/each}
  </nav>
  {#if analytics.error}<div role="alert" class="cm-error cm:mb-6">
      {errorMessage(analytics.error)}
    </div>{/if}
  {#if !project && !analytics.error}<div role="status" class="cm-empty">Loading analytics…</div>
  {:else if project}
    {#if route.section === 'overview'}<OverviewScreen {days} {events} />
    {:else if route.section === 'retention'}<RetentionScreen {cohorts} />
    {:else if route.section === 'funnels'}{#key route.projectId}<FunnelsScreen
          projectId={route.projectId}
          canAdmin={current?.canAdmin ?? false}
          definitions={project.funnels}
          reports={funnels}
          onchanged={reloadAnalytics}
        />{/key}
    {:else if route.section === 'segments'}{#key route.projectId}<SegmentsScreen
          projectId={route.projectId}
          canAdmin={current?.canAdmin ?? false}
          funnels={project.funnels}
          {createWebhook}
        />{/key}
    {:else if route.section === 'settings'}
      {#if current?.canAdmin}{#key route.projectId}<SettingsScreen
            projectId={route.projectId}
            settings={project.settings as Settings}
            onchanged={reloadAnalytics}
            {createDestination}
            {allowAccessKeys}
            {maxRetentionDays}
          />{/key}
      {:else}<div class="cm-surface cm:p-6">
          <h2>Project administrator access required.</h2>
          <p class="cm:mt-2 cm:text-sm cm:text-muted">You can view reports for this project.</p>
        </div>{/if}
    {/if}
  {/if}
{:else if projectsQuery.data && !projects.length && accessQuery.data}
  <div class="cm-empty">
    {accessQuery.data.viewer.canCreateProjects
      ? 'No projects yet. Create a project to start collecting events.'
      : 'No accessible projects.'}
  </div>
{:else if !projectsQuery.data && !projectsQuery.error}<div role="status" class="cm-empty">
    Loading projects…
  </div>{/if}
<footer
  class="cm:mt-10 cm:flex cm:justify-between cm:border-t cm:border-line cm:pt-5 cm:text-xs cm:text-muted"
>
  <span>Clickman</span><span>Days and cohorts use UTC.</span>
</footer>
