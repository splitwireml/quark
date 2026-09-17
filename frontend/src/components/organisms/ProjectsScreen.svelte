<script lang="ts">
  import Icon from '../atoms/Icon.svelte';

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

  const VIEW_STORAGE_KEY = 'quark.projects.view';
  let { projects, projectName, loading, creating, error, setProjectName, onCreate, onOpen }: Props = $props();
  let view = $state<'grid' | 'list'>(readView());
  let open = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  let ready = $derived(!!projectName.trim() && !creating);

  $effect(() => { if (open) input?.focus(); });
  function close() { open = false; setProjectName(''); }
  function readView(): 'grid' | 'list' {
    try { return localStorage.getItem(VIEW_STORAGE_KEY) === 'list' ? 'list' : 'grid'; } catch { return 'grid'; }
  }
  function setView(next: 'grid' | 'list') {
    view = next;
    try { localStorage.setItem(VIEW_STORAGE_KEY, next); } catch { /* private mode: the choice just does not persist */ }
  }
</script>

<section class="screen" aria-labelledby="projects-title">
  <div class="content">
    <header>
      <div>
        <h1 id="projects-title">Projects</h1>
        <p>Keep sources and Views together in a local workspace.</p>
      </div>

      <form class="creator" class:open onsubmit={onCreate}>
        <span class="mark" aria-hidden="true"><Icon name="plus" size={14} /></span>
        {#if open}
          <input
            bind:this={input}
            id="project-name"
            name="name"
            aria-label="Project name"
            value={projectName}
            oninput={(event) => setProjectName(event.currentTarget.value)}
            placeholder="Project name"
            autocomplete="off"
            disabled={creating}
            onkeydown={(event) => { if (event.key === 'Escape') close(); }}
            onblur={() => { if (!projectName.trim()) close(); }}
          />
          <button type="submit" class="go" class:ready aria-label="Create project" disabled={!ready}>
            <Icon name="arrow-right" size={14} />
          </button>
        {:else}
          <button type="button" class="trigger" onclick={() => open = true}>New project</button>
        {/if}
      </form>
    </header>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    {#if !loading && projects.length > 0}
      <div class="view-toggle" role="group" aria-label="Project layout">
        <span class="thumb" class:list={view === 'list'} aria-hidden="true"></span>
        <button type="button" class:active={view === 'grid'} aria-pressed={view === 'grid'} aria-label="Grid view" title="Grid view" onclick={() => setView('grid')}>
          <Icon name="grid" size={14} />
        </button>
        <button type="button" class:active={view === 'list'} aria-pressed={view === 'list'} aria-label="List view" title="List view" onclick={() => setView('list')}>
          <Icon name="list" size={14} />
        </button>
      </div>
    {/if}

    <div class="projects" aria-live="polite" aria-busy={loading}>
      {#if loading}
        <p class="state">Loading projects…</p>
      {:else if projects.length === 0}
        <p class="state">No projects yet. Create one to get started.</p>
      {:else}
        <ul class={view}>
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
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
  h1 { margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.01em; }
  p { margin: 0; }
  header p, .state { color: var(--muted); }
  header p { margin-top: 6px; }

  /* the trigger is the field: same box, widened and handed a caret.
     ponytail: the morph animates width on purpose — it is one control in a static header row,
     and transform would distort the label; revisit if the header ever animates alongside it. */
  .creator {
    flex: none;
    display: flex;
    align-items: center;
    gap: 7px;
    width: 148px;
    height: 32px;
    padding: 0 6px 0 10px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--control-border);
    background: var(--surface);
    transition: width 220ms cubic-bezier(0.22, 1, 0.36, 1), border-color 160ms ease, box-shadow 160ms ease;
  }
  .creator:hover:not(.open) { border-color: var(--faint); }
  .creator.open {
    width: 288px;
    border-color: var(--success);
    box-shadow: 0 1px 2px color-mix(in srgb, var(--success) 22%, transparent);
  }
  .mark { display: flex; flex: none; color: var(--glyph); transition: color 160ms ease, transform 220ms cubic-bezier(0.22, 1, 0.36, 1); }
  .creator.open .mark { color: var(--success); transform: rotate(90deg); }
  .trigger {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-2);
    cursor: pointer;
  }
  .creator input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    font-size: 12.5px;
    color: var(--ink);
  }
  .creator input:focus { outline: none; }
  .creator input::placeholder { color: var(--faint); }
  .go {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--faint);
    opacity: 0;
    transform: translateX(-4px);
    transition: opacity 160ms ease, transform 200ms cubic-bezier(0.22, 1, 0.36, 1), background 160ms ease, color 160ms ease;
    pointer-events: none;
  }
  .go.ready {
    background: var(--success);
    color: var(--ink);
    opacity: 1;
    transform: none;
    pointer-events: auto;
    cursor: pointer;
  }
  .go.ready:hover { background: color-mix(in srgb, var(--success) 86%, #000); }

  .view-toggle {
    position: relative;
    align-self: flex-end;
    display: inline-flex;
    gap: 2px;
    margin-bottom: -12px;
    padding: 2px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--line);
    background: var(--surface-3);
  }
  .view-toggle .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 26px;
    height: 22px;
    border-radius: var(--radius-md);
    border: 1px solid var(--control-border);
    background: var(--surface);
    transition: transform 180ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .view-toggle .thumb.list { transform: translateX(28px); }
  .view-toggle button {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 22px;
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--glyph);
    cursor: pointer;
    transition: color 140ms ease;
  }
  .view-toggle button:hover { color: var(--ink-2); }
  .view-toggle button.active { color: var(--ink); }

  .error { padding: 10px 12px; border: 1px solid var(--error); border-radius: var(--radius-lg); background: color-mix(in srgb, var(--error) 8%, var(--surface)); color: var(--error); font-size: 12.5px; }
  .projects { min-height: 80px; }
  .state { padding: 20px 0; font-size: 12.5px; }
  ul { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
  ul.grid { grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); }
  ul.list { grid-template-columns: 1fr; gap: 6px; }
  .project { width: 100%; min-width: 0; padding: 14px; display: flex; align-items: center; justify-content: space-between; gap: 12px; border: 1px solid var(--line); border-radius: var(--radius-xl); background: var(--surface); text-align: left; transition: border-color 120ms ease, background 120ms ease; }
  ul.grid .project { flex-direction: column; align-items: flex-start; gap: 5px; padding: 12px 14px 13px; }
  ul.list .project { padding: 10px 14px; border-radius: var(--radius-lg); }
  .project:hover { border-color: var(--control-border); background: var(--surface-3); }
  .project strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .project span { flex: none; font: 10px var(--font-mono); color: var(--faint); }

  @media (prefers-reduced-motion: reduce) {
    .creator, .mark, .go, .view-toggle .thumb { transition: none; }
    .creator.open .mark { transform: none; }
  }
  @media (max-width: 520px) {
    .screen { padding-top: 36px; }
    header { flex-direction: column; align-items: stretch; }
    .creator, .creator.open { width: 100%; }
    ul.grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>
