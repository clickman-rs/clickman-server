<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { setCachebay, createQuery, createMutation } from 'cachebay/svelte';
  import { Button, Stat, ActivityChart } from '@clickman/ui';
  import { ProjectsDocument, AnalyticsDocument, CreateProjectDocument, RefreshAnalyticsDocument, type ProjectsQuery, type AnalyticsQuery, type AnalyticsQueryVariables, type CreateProjectMutation, type CreateProjectMutationVariables, type RefreshAnalyticsMutation, type RefreshAnalyticsMutationVariables } from '@clickman/graphql-types';
  import { createCache } from './lib/cache';
  import { number, percentage, type ActiveDay, type Cohort, type EventTotal, type FunnelReport, type Settings } from './lib/reports';
  import Funnels from './Funnels.svelte';
  import SettingsPanel from './Settings.svelte';

  let { token, onlogout }: { token: string; onlogout: () => void } = $props();
  setCachebay(createCache(untrack(() => token)));
  const projectsQuery = createQuery<ProjectsQuery>({ query: ProjectsDocument });
  let selected = $state('');
  let tab = $state<'overview' | 'retention' | 'funnels' | 'settings'>('overview');
  let adding = $state(false);
  let name = $state('');
  let slug = $state('');
  let notice = $state('');
  const analytics = createQuery<AnalyticsQuery, AnalyticsQueryVariables>({ query: AnalyticsDocument, variables: () => ({ id: selected }), enabled: () => Boolean(selected) });
  const create = createMutation<CreateProjectMutation, CreateProjectMutationVariables>({ query: CreateProjectDocument });
  const refresh = createMutation<RefreshAnalyticsMutation, RefreshAnalyticsMutationVariables>({ query: RefreshAnalyticsDocument });
  let projects = $derived(projectsQuery.data?.projects.edges.map(edge => edge.node) ?? []);
  let current = $derived(projects.find(p => p.id === selected));
  let reports = $derived(analytics.data?.project.reports ?? []);
  let days = $derived(((reports.find(r => r.key === 'actives')?.result as { days?: ActiveDay[] } | undefined)?.days) ?? []);
  let cohorts = $derived(((reports.find(r => r.key === 'retention')?.result as { cohorts?: Cohort[] } | undefined)?.cohorts) ?? []);
  let events = $derived(((reports.find(r => r.key === 'events')?.result as { events?: EventTotal[] } | undefined)?.events) ?? []);
  let funnels = $derived(reports.filter(r => r.key.startsWith('funnel:')).map(r => r.result as FunnelReport));
  let today = $derived(days.at(-1));
  let totalEvents = $derived(events.reduce((sum, e) => sum + e.events, 0));
  let computed = $derived(reports[0]?.computedAt);
  $effect(() => { if (projects.length && !projects.some(p => p.id === selected)) selected = projects[0].id; });
  $effect(() => { if (tab === 'settings' && !current?.canAdmin) tab = 'overview'; });
  onMount(() => { const timer = setInterval(() => { if (selected && !analytics.isFetching) void analytics.refetch(); }, 60_000); return () => clearInterval(timer); });

  async function addProject(event: SubmitEvent) {
    event.preventDefault(); notice = '';
    const result = await create.execute({ slug: slug.trim(), name: name.trim() });
    if (result.error) { notice = result.error.message; return; }
    await projectsQuery.refetch();
    selected = result.data!.createProject.id; adding = false; name = ''; slug = '';
  }
  async function updateReports() {
    notice = '';
    const result = await refresh.execute({ projectId: selected });
    if (result.error) notice = result.error.message;
    else await analytics.refetch();
  }
</script>

<div class="min-h-screen">
  <header class="border-b border-line bg-white px-6 lg:px-10">
    <div class="mx-auto flex h-20 max-w-7xl items-center justify-between gap-4">
      <div class="flex items-center gap-3"><span class="flex h-9 w-9 items-center justify-center rounded-xl bg-brand font-semibold text-white">c.</span><span class="text-xl font-semibold tracking-tight">clickman</span><span class="ml-4 hidden border-l border-line pl-6 text-sm text-muted md:block">Product analytics</span></div>
      <div class="flex items-center gap-4"><span class="hidden text-xs text-muted sm:inline">Your database. Your data.</span><Button variant="ghost" onclick={onlogout}>Sign out</Button></div>
    </div>
  </header>
  <main class="mx-auto max-w-7xl p-6 lg:px-10 lg:py-8">
    {#if projectsQuery.error}<div role="alert" class="error">{projectsQuery.error.message}</div>{/if}
    {#if notice}<div role="alert" class="error mb-5">{notice}</div>{/if}
    <div class="mb-8 flex flex-wrap items-center justify-between gap-4">
      <div class="flex items-center gap-3"><span class="section-label">Project</span>{#if projects.length}<select aria-label="Project" class="max-w-64" bind:value={selected}>{#each projects as project}<option value={project.id}>{project.name}</option>{/each}</select>{/if}</div>
      {#if projectsQuery.data?.viewer.canCreateProjects}<Button variant="secondary" onclick={() => adding = !adding}>+ New project</Button>{/if}
    </div>
    {#if adding || (!projects.length && projectsQuery.data?.viewer.canCreateProjects)}
      <form onsubmit={addProject} class="surface mb-8 grid gap-5 p-6 sm:grid-cols-2">
        <div class="sm:col-span-2"><h2>Create a project</h2><p class="mt-1 text-sm text-muted">Each project has its own events, reports and access keys.</p></div>
        <label>Project name<input bind:value={name} required placeholder="My app" maxlength="200" /></label>
        <label>Project slug<input bind:value={slug} required placeholder="my-app" pattern="[a-z0-9_-]+" maxlength="64" /></label>
        <div class="sm:col-span-2"><Button type="submit" disabled={create.isFetching}>Create project</Button></div>
      </form>
    {/if}
    {#if current}
      <div class="flex flex-wrap items-center justify-between gap-5">
        <div><p class="section-label mb-2">{current.slug}</p><h1>{current.name}</h1><p class="mt-2 text-sm text-muted">Activity, retention and the paths people take.</p></div>
        <div class="flex items-center gap-4"><span class="text-xs text-muted">{computed ? `Updated ${new Date(computed).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}` : 'Waiting for first report'}</span>{#if current.canAdmin}<Button variant="secondary" disabled={refresh.isFetching} onclick={updateReports}>{refresh.isFetching ? 'Updating…' : 'Refresh reports'}</Button>{/if}</div>
      </div>
      <nav aria-label="Analytics sections" class="mb-7 mt-7 flex gap-7 border-b border-line">
        {#each ['overview', 'retention', 'funnels', ...(current.canAdmin ? ['settings'] : [])] as section}<button class:active={tab === section} class="tab capitalize" onclick={() => tab = section as typeof tab}>{section}</button>{/each}
      </nav>
      {#if analytics.error}<div role="alert" class="error mb-6">{analytics.error.message}</div>{/if}
      {#if analytics.isFetching && !analytics.data}<div class="empty">Loading analytics…</div>{:else}
        {#if tab === 'overview'}
          <div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4"><Stat label="Daily active users" value={number(today?.dau ?? 0)} detail="Today · identified users" /><Stat label="Weekly active users" value={number(today?.wau ?? 0)} detail="Last 7 days" /><Stat label="Monthly active users" value={number(today?.mau ?? 0)} detail="Last 30 days" /><Stat label="Events" value={number(totalEvents)} detail="Last 30 days · all events" /></div>
          <ActivityChart {days} />
          <section class="surface mt-6 p-6"><div class="mb-4 flex justify-between"><h2>Top events</h2><span class="text-xs text-muted">Last 30 days</span></div>{#if events.length}<div class="overflow-x-auto"><table><thead><tr><th>Event</th><th class="text-right">Events</th><th class="text-right">Users</th><th class="pl-8">Share</th></tr></thead><tbody>{#each events as event}<tr><td class="font-medium">{event.event}</td><td class="text-right tabular-nums">{number(event.events)}</td><td class="text-right tabular-nums">{number(event.actors)}</td><td class="w-1/4 pl-8"><div class="flex items-center gap-3"><div class="h-1.5 flex-1 rounded bg-brand-light"><div class="h-1.5 rounded bg-brand" style:width={`${Math.max(1, event.events / Math.max(1, totalEvents) * 100)}%`}></div></div><span class="w-10 text-right text-xs text-muted">{percentage(event.events, totalEvents)}</span></div></td></tr>{/each}</tbody></table></div>{:else}<div class="empty">No events yet. Create a source key in Settings and connect your application.</div>{/if}</section>
        {:else if tab === 'retention'}
          <section class="surface p-6"><h2>Cohort retention</h2><p class="mb-5 mt-1 text-sm text-muted">Users who return exactly 1, 7 and 30 days after their first activity.</p>{#if cohorts.length}<div class="overflow-x-auto"><table><thead><tr><th>First active day</th><th class="text-right">Users</th><th class="text-center">Day 1</th><th class="text-center">Day 7</th><th class="text-center">Day 30</th></tr></thead><tbody>{#each [...cohorts].reverse() as cohort}<tr><td>{cohort.day}</td><td class="text-right tabular-nums">{number(cohort.size)}</td>{#each [cohort.d1, cohort.d7, cohort.d30] as count}<td class="text-center"><span class="inline-block min-w-16 rounded px-3 py-2 font-medium" style:background={count === null ? '#f5f8f6' : `rgba(20,110,89,${.05 + (cohort.size ? count / cohort.size : 0) * .2})`}>{percentage(count, cohort.size)}</span></td>{/each}</tr>{/each}</tbody></table></div>{:else}<div class="empty">Retention appears after identified users start sending events.</div>{/if}</section>
        {:else if tab === 'funnels'}
          {#key selected}<Funnels projectId={selected} canAdmin={current.canAdmin} definitions={analytics.data?.project.funnels ?? []} reports={funnels} onchanged={() => analytics.refetch()} />{/key}
        {:else if tab === 'settings' && current.canAdmin && analytics.data}
          {#key selected}<SettingsPanel projectId={selected} settings={analytics.data.project.settings as Settings} onchanged={() => analytics.refetch()} />{/key}
        {/if}
      {/if}
    {:else if !projectsQuery.isFetching && !projectsQuery.error && !projectsQuery.data?.viewer.canCreateProjects}
      <div class="empty">No accessible projects.</div>
    {/if}
    <footer class="mt-10 flex justify-between border-t border-line pt-5 text-xs text-muted"><span>Clickman</span><span>Days and cohorts use UTC.</span></footer>
  </main>
</div>
