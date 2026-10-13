<script lang="ts">
  import { createMutation } from 'cachebay/svelte';
  import { Button } from '../ui/index';
  import { dataOrThrow, errorMessage } from '../graphql/errors';
  import {
    CreateProjectDocument,
    type CreateProjectMutation,
    type CreateProjectMutationVariables,
    type ProjectsQuery,
  } from '../graphql/generated';

  let {
    projects,
    selected,
    canCreate,
    onselect,
    oncreated,
  }: {
    projects: ProjectsQuery['projects']['edges'][number]['node'][];
    selected: string | null;
    canCreate: boolean;
    onselect: (id: string) => void;
    oncreated: (id: string) => Promise<void>;
  } = $props();
  const create = createMutation<CreateProjectMutation, CreateProjectMutationVariables>({
    query: CreateProjectDocument,
  });
  let adding = $state(false);
  let name = $state('');
  let slug = $state('');
  let busy = $state(false);
  let error = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true;
    error = '';
    try {
      const result = dataOrThrow<CreateProjectMutation>(
        await create.execute({ name: name.trim(), slug: slug.trim() }),
      );
      name = slug = '';
      adding = false;
      await oncreated(result.createProject.id);
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = false;
    }
  }
</script>

<div class="cm:mb-8 cm:flex cm:flex-wrap cm:items-center cm:justify-between cm:gap-4">
  <div class="cm:flex cm:min-w-0 cm:max-w-full cm:items-center cm:gap-3">
    <span class="cm-section-label cm:shrink-0">Project</span>
    {#if projects.length}<select
        aria-label="Project"
        class="cm:min-w-0 cm:max-w-64"
        value={selected ?? ''}
        disabled={busy}
        onchange={(event) => onselect(event.currentTarget.value)}
      >
        {#if !selected}<option value="" disabled>Choose a project</option>{/if}
        {#each projects as project (project.id)}<option value={project.id}>{project.name}</option
          >{/each}
      </select>{/if}
  </div>
  {#if canCreate}<Button
      variant="secondary"
      disabled={busy}
      onclick={() => {
        adding = !adding;
        error = '';
      }}>+ New project</Button
    >{/if}
</div>
{#if error}<div role="alert" class="cm-error cm:mb-5">{error}</div>{/if}
{#if canCreate && (adding || !projects.length)}
  <form onsubmit={submit} class="cm-surface cm:mb-8 cm:grid cm:gap-5 cm:p-6 cm:sm:grid-cols-2">
    <div class="cm:sm:col-span-2">
      <h2>Create a project</h2>
      <p class="cm:mt-1 cm:text-sm cm:text-muted">
        Each project has its own events, reports and access keys.
      </p>
    </div>
    <label
      >Project name<input
        bind:value={name}
        disabled={busy}
        required
        maxlength="200"
        placeholder="My app"
      /></label
    >
    <label
      >Project slug<input
        bind:value={slug}
        disabled={busy}
        required
        maxlength="64"
        pattern="[a-z0-9_-]+"
        placeholder="my-app"
      /></label
    >
    <div class="cm:sm:col-span-2">
      <Button type="submit" disabled={busy}>{busy ? 'Creating…' : 'Create project'}</Button>
    </div>
  </form>
{/if}
