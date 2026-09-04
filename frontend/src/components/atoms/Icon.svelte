<script lang="ts">
  import type { IconName } from '../../lib/icons';

  type Props = { name: IconName; size?: number };
  let { name, size = 16 }: Props = $props();
  const paths: Record<IconName, string> = {
    upload: 'M8 10.5V2.5M8 2.5 5 5.5M8 2.5l3 3M2.5 10v2.5a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V10',
    download: 'M8 2.5V10M8 10 5 7M8 10l3-3M2.5 10v2.5a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V10',
    database: 'M13 4c0 1.1-2.24 2-5 2S3 5.1 3 4s2.24-2 5-2 5 .9 5 2ZM3 4v8c0 1.1 2.24 2 5 2s5-.9 5-2V4M3 8c0 1.1 2.24 2 5 2s5-.9 5-2',
    chevron: 'm6 3.5 4.5 4.5L6 12.5',
    file: 'M9 2H4.5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1V5.5L9 2ZM9 2v3.5h3.5',
    plus: 'M8 3.5v9M3.5 8h9',
    'arrow-right': 'M3 8h10M9 4l4 4-4 4',
    grid: 'M2.75 2.75h4v4h-4zM9.25 2.75h4v4h-4zM2.75 9.25h4v4h-4zM9.25 9.25h4v4h-4z',
    list: 'M2.75 4h10.5M2.75 8h10.5M2.75 12h10.5',
    columns: 'M4 2.75v10.5M8 2.75v10.5M12 2.75v10.5',
    join: 'M6.25 4.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7ZM9.75 4.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z',
    sigma: 'M11 3.5H5l3.4 4.5L5 12.5h6',
    duplicate: 'M5.75 5.75h7v7h-7zM10.25 5.75V3.25h-7v7h2.5',
    spacing: 'M2.75 3h10.5M2.75 13h10.5M8 5.75v4.5M6.6 7.15 8 5.75l1.4 1.4M6.6 8.85 8 10.25l1.4-1.4',
    compare: 'M8 2.75v10.5M5.25 5.5H2.75v5h2.5M10.75 5.5h2.5v5h-2.5',
    'density-compact': 'M3 4h10M3 6.67h10M3 9.33h10M3 12h10',
    'density-default': 'M3 4h10M3 8h10M3 12h10',
    'density-comfortable': 'M3 4.5h10M3 11.5h10',
    expand: 'M6 2.75H2.75V6M10 2.75h3.25V6M6 13.25H2.75V10M10 13.25h3.25V10',
    collapse: 'M2.75 6H6V2.75M13.25 6H10V2.75M2.75 10H6v3.25M13.25 10H10v3.25',
    braces: 'M6.5 2.5C5 2.5 5 3.9 5 5.2c0 1.3-.5 1.8-1.5 2.8 1 1 1.5 1.5 1.5 2.8 0 1.3 0 2.7 1.5 2.7M9.5 2.5c1.5 0 1.5 1.4 1.5 2.7 0 1.3.5 1.8 1.5 2.8-1 1-1.5 1.5-1.5 2.8 0 1.3 0 2.7-1.5 2.7',
    link: 'M6.5 9.5a2.5 2.5 0 0 0 3.54 0l2.46-2.46a2.5 2.5 0 0 0-3.54-3.54l-.9.9M9.5 6.5a2.5 2.5 0 0 0-3.54 0L3.5 8.96a2.5 2.5 0 0 0 3.54 3.54l.9-.9',
  };
</script>

<svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
  <path d={paths[name]} />
</svg>

<style>
  svg { flex: none; display: block; }
</style>
