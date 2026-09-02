<script lang="ts">
  import Button from '../atoms/Button.svelte';
  import TextInput from '../atoms/TextInput.svelte';

  type Project = { id: string; name: string; source_count: number };
  type Props = {
    projects: Project[];
    projectName: string;
    loading: boolean;
    creating: boolean;
    error: string;
    setProjectName: (name: string) => void;
    onCreate: (event: SubmitEvent) => void;
    onOpen: (id: string) => void;
  };

  let { projects, projectName, loading, creating, error, setProjectName, onCreate, onOpen }: Props = $props();
</script>

<section class="screen" aria-labelledby="projects-title">
  <div class="content">
    <header>
      <h1 id="projects-title">Projects</h1>
      <p>Keep sources and Views together in a local workspace.</p>
    </header>

    <form onsubmit={onCreate}>
      <label for="project-name">New project</label>
      <div class="create-row">
        <TextInput
          id="project-name"
          name="name"
          value={projectName}
          oninput={(event: Event) => setProjectName((event.currentTarget as HTMLInputElement).value)}
          placeholder="Project name"
          autocomplete="off"
          disabled={creating}
        />
        <Button type="submit" variant="primary" disabled={creating || !projectName.trim()}>
          {creating ? 'Creating…' : 'Create'}
        </Button>
      </div>
    </form>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="projects" aria-live="polite" aria-busy={loading}>
      {#if loading}
        <p class="state">Loading projects…</p>
      {:else if projects.length === 0}
        <p class="state">No projects yet. Create one to get started.</p>
      {:else}
        <ul>
          {#each projects as project (project.id)}
            <li>
              <button type="button" class="project" onclick={() => onOpen(project.id)}>
                <strong title={project.name}>{project.name}</strong>
                <span>{project.source_count} {project.source_count === 1 ? 'source' : 'sources'}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</section>

<style>
  .screen { height: 100%; overflow-y: auto; padding: 64px 24px; }
  .content { width: min(680px, 100%); margin: 0 auto; display: flex; flex-direction: column; gap: 24px; }
  header { display: flex; flex-direction: column; gap: 6px; }
  h1 { margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 0; }
  header p, .state { color: var(--muted); }
  form { display: flex; flex-direction: column; gap: 7px; }
  label { font-size: 12.5px; font-weight: 500; color: var(--ink-2); }
  .create-row { display: flex; align-items: center; gap: 8px; }
  .create-row :global(.field) { flex: 1; height: 30px; }
  .error { padding: 10px 12px; border: 1px solid var(--error); border-radius: var(--radius-lg); background: color-mix(in srgb, var(--error) 8%, var(--surface)); color: var(--error); font-size: 12.5px; }
  .projects { min-height: 80px; }
  .state { padding: 20px 0; font-size: 12.5px; }
  ul { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin: 0; padding: 0; list-style: none; }
  .project { width: 100%; min-width: 0; padding: 14px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border: 1px solid var(--line); border-radius: var(--radius-xl); background: var(--surface); text-align: left; transition: border-color 120ms ease, background 120ms ease; }
  .project:hover { border-color: var(--control-border); background: var(--surface-3); }
  .project strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .project span { flex: none; font: 10px var(--font-mono); color: var(--muted-2); }
  @media (max-width: 520px) {
    .screen { padding-top: 36px; }
    ul { grid-template-columns: 1fr; }
  }
</style>
