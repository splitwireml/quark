<script lang="ts">
  import { clampAbsoluteRow, thumbGeometry } from '../../lib/row-scrollbar';

  type Props = {
    // Absolute dataset rows; fractions allowed since scroll positions are continuous.
    totalRows: number;
    totalLabel: string;
    firstVisibleRow: number;
    visibleRows: number;
    trackHeight: number;
    // Id of the scrolled region, for the scrollbar role's required aria-controls.
    controls: string;
    disabled: boolean;
    // Absolute dataset row to reveal; the caller changes pages when it leaves the loaded page.
    onSeek: (absoluteRow: number) => void;
  };

  let { totalRows, totalLabel, firstVisibleRow, visibleRows, trackHeight, controls, disabled, onSeek }: Props = $props();

  // Direct manipulation only: the thumb tracks the pointer 1:1 with no animation,
  // so there is no motion to gate behind prefers-reduced-motion.
  let track = $state<HTMLDivElement | null>(null);
  let dragFirst = $state<number | null>(null);
  let dragGrab = $state(0);

  const shownFirst = $derived(dragFirst ?? firstVisibleRow);
  const geometry = $derived(thumbGeometry(trackHeight, totalRows, shownFirst, visibleRows));
  const maxFirst = $derived(Math.max(0, totalRows - visibleRows));
  const firstLabel = $derived(Math.floor(Math.min(Math.max(shownFirst, 0), Math.max(0, totalRows - 1))) + 1);
  const lastLabel = $derived(Math.min(totalRows, Math.ceil(shownFirst + visibleRows)));
  const valueText = $derived(`Rows ${firstLabel.toLocaleString()} to ${lastLabel.toLocaleString()} of ${totalLabel}`);

  function trackRect(): DOMRect | null { return track?.getBoundingClientRect() ?? null; }

  function startThumbDrag(event: PointerEvent) {
    if (disabled || event.button !== 0 || !track) return;
    event.preventDefault();
    event.stopPropagation();
    const thumb = event.currentTarget as HTMLDivElement;
    dragGrab = event.clientY - thumb.getBoundingClientRect().top;
    dragFirst = Math.min(Math.max(firstVisibleRow, 0), maxFirst);
    thumb.setPointerCapture(event.pointerId);
  }

  function moveThumbDrag(event: PointerEvent) {
    if (dragFirst === null) return;
    const rect = trackRect();
    if (!rect) return;
    const travel = Math.max(0, rect.height - geometry.thumbHeight);
    const offset = event.clientY - rect.top - dragGrab;
    dragFirst = travel <= 0 ? 0 : (Math.min(Math.max(offset, 0), travel) / travel) * maxFirst;
  }

  function endThumbDrag() {
    if (dragFirst === null) return;
    const target = clampAbsoluteRow(Math.round(dragFirst), totalRows);
    dragFirst = null;
    onSeek(target);
  }

  function jumpTrack(event: PointerEvent) {
    if (disabled || event.button !== 0) return;
    const rect = trackRect();
    if (!rect || rect.height <= 0) return;
    const fraction = Math.min(Math.max((event.clientY - rect.top) / rect.height, 0), 1);
    onSeek(clampAbsoluteRow(Math.round(fraction * totalRows - visibleRows / 2), totalRows));
  }

  function seekKey(event: KeyboardEvent) {
    if (disabled) return;
    const step = Math.max(1, Math.round(visibleRows));
    const targets: Record<string, number> = {
      ArrowUp: firstVisibleRow - 1,
      ArrowDown: firstVisibleRow + 1,
      PageUp: firstVisibleRow - step,
      PageDown: firstVisibleRow + step,
      Home: 0,
      End: totalRows - 1,
    };
    if (!(event.key in targets)) return;
    event.preventDefault();
    onSeek(clampAbsoluteRow(Math.round(targets[event.key]), totalRows));
  }
</script>

{#if geometry.scrollable}
  <!-- The track itself is a pointer-only shortcut; the thumb below is the keyboard control. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={track}
    class="row-scrollbar"
    onpointerdown={(event) => { if (event.target === track) jumpTrack(event); }}
  >
    <div
      class="thumb"
      role="scrollbar"
      aria-orientation="vertical"
      aria-label="Scroll all rows"
      aria-controls={controls}
      aria-valuemin={1}
      aria-valuemax={totalRows}
      aria-valuenow={firstLabel}
      aria-valuetext={valueText}
      aria-disabled={disabled}
      tabindex={disabled ? -1 : 0}
      title={valueText}
      style:top={`${geometry.thumbTop}px`}
      style:height={`${geometry.thumbHeight}px`}
      onpointerdown={startThumbDrag}
      onpointermove={moveThumbDrag}
      onpointerup={endThumbDrag}
      onpointercancel={() => { dragFirst = null; }}
      onkeydown={seekKey}
    ></div>
    {#if dragFirst !== null}
      <div class="thumb-tip" style:top={`${geometry.thumbTop}px`}>
        Row {(Math.round(dragFirst) + 1).toLocaleString()} of {totalLabel}
      </div>
    {/if}
  </div>
{/if}

<style>
  .row-scrollbar {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 12px;
    z-index: 8;
    border-radius: 8px;
    touch-action: none;
  }
  .row-scrollbar:hover { background: color-mix(in srgb, var(--muted) 12%, transparent); }
  .thumb {
    position: absolute;
    left: 3px;
    right: 3px;
    border-radius: 999px;
    background: var(--faint);
    opacity: 0.55;
    cursor: grab;
    touch-action: none;
  }
  .thumb:hover { opacity: 0.85; }
  .thumb:active { cursor: grabbing; opacity: 1; }
  .thumb:focus-visible { outline: 2px solid var(--action); outline-offset: 1px; opacity: 1; }
  .thumb-tip {
    position: absolute;
    right: 16px;
    transform: translateY(-50%);
    padding: 3px 8px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    font: 11px var(--font-mono);
    color: var(--ink);
    white-space: nowrap;
    pointer-events: none;
  }
</style>
