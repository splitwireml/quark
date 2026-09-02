<script lang="ts">
  import PaginationControl from '../molecules/PaginationControl.svelte';

  type Props = {
    pageSizes: number[];
    pageSize: number;
    onChangePageSize: (event: Event) => void;
    rangeStart: string;
    rangeEnd: string;
    totalRows: string;
    page: number;
    pageInput: string;
    totalPages: number;
    loadingData: boolean;
    onPrev: () => void;
    onNext: () => void;
    onJump: (event: SubmitEvent) => void;
    setPageInput: (value: string) => void;
  };
  let { pageSizes, pageSize, onChangePageSize, rangeStart, rangeEnd, totalRows, page, pageInput, totalPages, loadingData, onPrev, onNext, onJump, setPageInput }: Props = $props();
</script>

<footer class="pagination" inert={loadingData}>
  <label>Rows per page
    <select value={pageSize} onchange={onChangePageSize}>
      {#each pageSizes as size (size)}<option value={size}>{size}</option>{/each}
    </select>
  </label>
  <span class="range">{rangeStart}–{rangeEnd} of {totalRows}</span>
  <PaginationControl {page} {pageInput} {totalPages} {loadingData} onprev={onPrev} onnext={onNext} onjump={onJump} {setPageInput} />
</footer>

<style>
  .pagination {
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    height: 40px;
    padding: 0 20px;
    border-top: 1px solid var(--line);
    background: var(--surface-2);
    font-size: 12px;
    color: var(--muted);
  }
  label { display: flex; align-items: center; gap: 6px; }
  select { height: 24px; border-radius: var(--radius-sm); border: 1px solid var(--control-border); padding: 0 6px; }
  .range { font-family: var(--font-mono); font-size: 11px; }
</style>
