<script lang="ts">
  import PreferenceOption from './PreferenceOption.svelte';
  import type { ThemePreference } from '../../lib/theme';

  let { value, selected, onSelect }: { value: ThemePreference; selected: boolean; onSelect: () => void } = $props();

  const copy: Record<ThemePreference, { title: string; detail: string }> = {
    system: { title: 'Match system', detail: 'Follow the light or dark setting of this computer, and change with it.' },
    light: { title: 'Light', detail: 'The daylight workspace: a pale canvas with dark data.' },
    dark: { title: 'Dark', detail: 'The same instrument panel on a dark canvas, for dim rooms and long sessions.' }
  };
</script>

<PreferenceOption name="color-scheme" {value} title={copy[value].title} detail={copy[value].detail} {selected} {onSelect}>
  <!-- The preview shows each scheme in its own colors rather than the tokens of the page it
       sits on, so Dark reads as dark while the app is still light. "Match system" shows both,
       split down the middle: the same window, twice. -->
  <div class="preview" aria-hidden="true">
    <div class="stack" class:split={value === 'system'}>
      <div class="mini" data-scheme={value === 'dark' ? 'dark' : 'light'}>
        <div class="title"><span></span><i></i><i></i></div>
        <div class="body">
          <div class="rail">{#each [0, 1, 2, 3] as row (row)}<u></u>{/each}</div>
          <div class="table">{#each [0, 1, 2, 3, 4] as row (row)}<div>{#each [0, 1, 2] as col (col)}<span><i style:width="{34 + ((row + col) % 3) * 20}%"></i></span>{/each}</div>{/each}</div>
        </div>
      </div>
      {#if value === 'system'}
        <div class="mini over" data-scheme="dark">
          <div class="title"><span></span><i></i><i></i></div>
          <div class="body">
            <div class="rail">{#each [0, 1, 2, 3] as row (row)}<u></u>{/each}</div>
            <div class="table">{#each [0, 1, 2, 3, 4] as row (row)}<div>{#each [0, 1, 2] as col (col)}<span><i style:width="{34 + ((row + col) % 3) * 20}%"></i></span>{/each}</div>{/each}</div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</PreferenceOption>

<style>
  .preview { height: 100%; padding: 20px 16px 0; }
  .stack { position: relative; height: 144px; }
  .mini {
    position: absolute; inset: 0;
    overflow: hidden;
    border: 1px solid var(--m-line-strong);
    border-radius: 6px 6px 0 0;
    background: var(--m-surface);
  }
  /* Both schemes have to be on screen at once, so the preview states its own colours
     instead of reading the tokens of the surface underneath it. */
  .mini[data-scheme='light'] { --m-surface: #FFFFFF; --m-surface-2: #F4F6F9; --m-surface-3: #FAFBFD; --m-line: #E6EAF0; --m-line-strong: #D5DBE4; --m-ink: #515D6B; --m-bar: #D5DBE4; }
  .mini[data-scheme='dark'] { --m-surface: #191E27; --m-surface-2: #161A22; --m-surface-3: #13171E; --m-line: #272E3A; --m-line-strong: #343D4B; --m-ink: #A6B2C1; --m-bar: #343D4B; }
  .over { clip-path: inset(0 0 0 50%); border-left: 1px solid var(--m-line-strong); }
  .title { display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 9px; background: var(--m-surface-2); border-bottom: 1px solid var(--m-line); }
  .title span { width: 38%; height: 4px; margin-right: auto; border-radius: 2px; background: var(--m-ink); opacity: .6; }
  .title i { width: 8px; height: 8px; border: 1px solid var(--m-line-strong); border-radius: 2px; }
  .body { display: grid; grid-template-columns: 34% 1fr; height: calc(100% - 26px); }
  .rail { display: flex; flex-direction: column; gap: 9px; padding: 11px 9px; background: var(--m-surface-3); border-right: 1px solid var(--m-line); }
  .rail u { height: 3px; border-radius: 1px; background: var(--m-bar); }
  .rail u:nth-child(2) { width: 72%; }
  .rail u:nth-child(3) { width: 84%; }
  .rail u:nth-child(4) { width: 60%; }
  .table > div { display: grid; grid-template-columns: repeat(3, 1fr); height: 20px; border-bottom: 1px solid var(--m-line); }
  .table > div:first-child { background: var(--m-surface-2); }
  .table span { display: flex; align-items: center; padding-left: 7px; border-right: 1px solid var(--m-line); }
  .table i { display: block; height: 3px; border-radius: 1px; background: var(--m-bar); }
</style>
