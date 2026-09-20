export type ChartTheme = 'single' | 'monotone' | 'multicolor';

export const chartThemeModes: readonly ChartTheme[] = ['single', 'monotone', 'multicolor'];

type ChartPaletteColors = {
  mark: string;
  strong: string;
  fill: string;
  ink: string;
  series: readonly string[];
  seriesFill: readonly string[];
};

export type ChartPalette = 'blue' | 'violet' | 'teal' | 'amber' | 'graphite' | 'ocean' | 'forest' | 'ember' | 'classic' | 'colorblind' | 'sunset';

export type ChartPaletteDefinition = {
  id: ChartPalette;
  title: string;
  detail: string;
  colors: ChartPaletteColors;
};

export const chartPalettes = {
  single: [
    {
      id: 'blue', title: 'Blue', detail: 'One clear workspace accent.',
      colors: {
        mark: '#C9DBFF', strong: '#1155F5', fill: '#EDF3FF', ink: '#0B3FBF',
        series: ['#1155F5', '#1155F5', '#1155F5', '#1155F5', '#1155F5', '#1155F5'],
        seriesFill: ['#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF']
      }
    },
    {
      id: 'violet', title: 'Violet', detail: 'A focused, cooler alternative.',
      colors: {
        mark: '#D8CCFF', strong: '#6B4CE6', fill: '#F1EEFF', ink: '#4B35A8',
        series: ['#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6'],
        seriesFill: ['#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF']
      }
    },
    {
      id: 'teal', title: 'Teal', detail: 'A calm accent for long scans.',
      colors: {
        mark: '#BFEAE2', strong: '#0F8F7D', fill: '#E8F8F4', ink: '#0A6F61',
        series: ['#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D'],
        seriesFill: ['#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2']
      }
    },
    {
      id: 'amber', title: 'Amber', detail: 'A warmer signal for charts.',
      colors: {
        mark: '#F4D6AB', strong: '#C45C16', fill: '#FFF4E6', ink: '#8D3E0C',
        series: ['#C45C16', '#C45C16', '#C45C16', '#C45C16', '#C45C16', '#C45C16'],
        seriesFill: ['#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB']
      }
    }
  ],
  monotone: [
    {
      id: 'graphite', title: 'Graphite', detail: 'Ink on paper with quiet contrast.',
      colors: {
        mark: '#C5CDD8', strong: '#1F2533', fill: '#EEF1F5', ink: '#1F2533',
        series: ['#1F2533', '#2B3444', '#515D6B', '#697786', '#8A97A6', '#A8B2BE'],
        seriesFill: ['#C5CDD8', '#D5DBE4', '#DEE3EA', '#E7EBF0', '#EEF1F5', '#F5F7F9']
      }
    },
    {
      id: 'ocean', title: 'Ocean', detail: 'One blue hue from deep to light.',
      colors: {
        mark: '#BDD2F4', strong: '#174EA6', fill: '#EEF4FF', ink: '#123D80',
        series: ['#174EA6', '#2A63B8', '#487AC5', '#6D93D2', '#91ACE0', '#B6C6EC'],
        seriesFill: ['#BDD2F4', '#C7D9F5', '#D2E0F7', '#DDE7F9', '#E8EFFB', '#F1F5FC']
      }
    },
    {
      id: 'forest', title: 'Forest', detail: 'A measured green scale for distributions.',
      colors: {
        mark: '#BDE2CC', strong: '#1E6B45', fill: '#EBF7EF', ink: '#155334',
        series: ['#1E6B45', '#2E7D52', '#4B9369', '#6AA77F', '#8AB995', '#B1D2B9'],
        seriesFill: ['#BDE2CC', '#C8E6D2', '#D4EBDD', '#DFEFE4', '#EAF4ED', '#F2F8F3']
      }
    },
    {
      id: 'ember', title: 'Ember', detail: 'A warm scale with a precise edge.',
      colors: {
        mark: '#F3C9A1', strong: '#A64716', fill: '#FFF1E6', ink: '#7D350D',
        series: ['#A64716', '#B95822', '#C96B33', '#D78452', '#E39F73', '#F0C1A0'],
        seriesFill: ['#F3C9A1', '#F5D1AE', '#F7DABD', '#F9E2CC', '#FBEADF', '#FDF3EC']
      }
    }
  ],
  multicolor: [
    {
      id: 'classic', title: 'Classic', detail: 'Blue, teal, orange, violet, rose, green.',
      colors: {
        mark: '#8FB4FF', strong: '#1155F5', fill: '#E8F0FF', ink: '#0B3FBF',
        series: ['#1155F5', '#0F9D8A', '#C45C16', '#6B4CE6', '#C43B5C', '#2A8F3A'],
        seriesFill: ['#C9DBFF', '#BFEAE2', '#F4D6AB', '#D8CCFF', '#F3C8D3', '#C9E7CC']
      }
    },
    {
      id: 'colorblind', title: 'Okabe–Ito', detail: 'Distinct blue, orange, green, and violet hues.',
      colors: {
        mark: '#8CC6E2', strong: '#0072B2', fill: '#E8F4FA', ink: '#005A8D',
        series: ['#0072B2', '#E69F00', '#009E73', '#CC79A7', '#D55E00', '#56B4E9'],
        seriesFill: ['#BFDDEE', '#F8DFAC', '#BCE8DA', '#E8C9DD', '#F2C4A5', '#C9EAF7']
      }
    },
    {
      id: 'sunset', title: 'Sunset', detail: 'A softer set of distinct warm hues.',
      colors: {
        mark: '#9CC7C8', strong: '#264653', fill: '#EDF5F4', ink: '#1E3A46',
        series: ['#264653', '#2A9D8F', '#E9C46A', '#F4A261', '#E76F51', '#8D5A97'],
        seriesFill: ['#BFD4D7', '#BFE4DF', '#F7E7B6', '#F9D4B3', '#F6C0B8', '#D9C5DD']
      }
    }
  ]
} as const;

export type ChartThemeSelections = Record<ChartTheme, ChartPalette>;
export type ChartThemePreferences = { mode: ChartTheme; palettes: ChartThemeSelections };

export const defaultChartThemePreferences: ChartThemePreferences = {
  mode: 'single',
  palettes: { single: 'blue', monotone: 'graphite', multicolor: 'classic' }
};

const legacyThemes: Record<string, ChartTheme> = {
  primary: 'single', monotone: 'monotone', rich: 'multicolor'
};

function legacyThemeFor(value: unknown): ChartTheme | undefined {
  return typeof value === 'string' && Object.prototype.hasOwnProperty.call(legacyThemes, value) ? legacyThemes[value] : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function isChartTheme(value: unknown): value is ChartTheme {
  return typeof value === 'string' && chartThemeModes.includes(value as ChartTheme);
}

export function isChartPalette(mode: ChartTheme, value: unknown): value is ChartPalette {
  return typeof value === 'string' && chartPalettes[mode].some((palette) => palette.id === value);
}

export function paletteOptions(mode: ChartTheme): readonly ChartPaletteDefinition[] {
  return chartPalettes[mode];
}

export function chartPaletteFor(mode: ChartTheme, id: ChartPalette) {
  return chartPalettes[mode].find((palette) => palette.id === id) ?? chartPalettes[mode][0];
}

export function normalizeChartThemePreferences(value: unknown): ChartThemePreferences {
  const fallback = {
    mode: defaultChartThemePreferences.mode,
    palettes: { ...defaultChartThemePreferences.palettes }
  };
  if (!isRecord(value)) return fallback;
  const legacyMode = legacyThemeFor(value.mode);
  const mode = isChartTheme(value.mode) ? value.mode : legacyMode ?? fallback.mode;
  const stored = isRecord(value.palettes) ? value.palettes : {};
  const palettes = { ...fallback.palettes };
  for (const candidate of chartThemeModes) {
    const palette = stored[candidate];
    if (isChartPalette(candidate, palette)) palettes[candidate] = palette;
  }
  if (typeof value.palette === 'string' && isChartPalette(mode, value.palette)) palettes[mode] = value.palette;
  return { mode, palettes };
}

export function readChartThemePreferences(raw: string | null): ChartThemePreferences {
  if (!raw) return normalizeChartThemePreferences(null);
  const mode = legacyThemeFor(raw);
  if (mode) {
    return { mode, palettes: { ...defaultChartThemePreferences.palettes } };
  }
  try {
    return normalizeChartThemePreferences(JSON.parse(raw));
  } catch {
    return normalizeChartThemePreferences(null);
  }
}

export function serializeChartThemePreferences(preferences: ChartThemePreferences): string {
  return JSON.stringify(normalizeChartThemePreferences(preferences));
}

export function chartThemeCssVariables(preferences: ChartThemePreferences): Record<string, string> {
  const selected = chartPaletteFor(preferences.mode, preferences.palettes[preferences.mode]);
  const variables: Record<string, string> = {
    '--chart-mark': selected.colors.mark,
    '--chart-mark-strong': selected.colors.strong,
    '--chart-mark-fill': selected.colors.fill,
    '--chart-mark-ink': selected.colors.ink
  };
  selected.colors.series.forEach((color, index) => {
    variables[`--chart-series-${index + 1}`] = color;
    variables[`--chart-series-${index + 1}-fill`] = selected.colors.seriesFill[index] ?? selected.colors.mark;
  });
  return variables;
}
