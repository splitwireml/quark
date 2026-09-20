import type { ColorScheme } from './theme';

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
  dark: ChartPaletteColors;
};

export const chartPalettes = {
  single: [
    {
      id: 'blue', title: 'Blue', detail: 'One clear workspace accent.',
      colors: {
        mark: '#C9DBFF', strong: '#1155F5', fill: '#EDF3FF', ink: '#0B3FBF',
        series: ['#1155F5', '#1155F5', '#1155F5', '#1155F5', '#1155F5', '#1155F5'],
        seriesFill: ['#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF', '#C9DBFF']
      },
      dark: {
        mark: '#37538F', strong: '#5C8DFF', fill: '#17233C', ink: '#A6C2FF',
        series: ['#5C8DFF', '#5C8DFF', '#5C8DFF', '#5C8DFF', '#5C8DFF', '#5C8DFF'],
        seriesFill: ['#2C4276', '#2C4276', '#2C4276', '#2C4276', '#2C4276', '#2C4276']
      }
    },
    {
      id: 'violet', title: 'Violet', detail: 'A focused, cooler alternative.',
      colors: {
        mark: '#D8CCFF', strong: '#6B4CE6', fill: '#F1EEFF', ink: '#4B35A8',
        series: ['#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6', '#6B4CE6'],
        seriesFill: ['#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF', '#D8CCFF']
      },
      dark: {
        mark: '#4C3B8C', strong: '#9B7DFF', fill: '#201A3A', ink: '#C7B6FF',
        series: ['#9B7DFF', '#9B7DFF', '#9B7DFF', '#9B7DFF', '#9B7DFF', '#9B7DFF'],
        seriesFill: ['#3C2F70', '#3C2F70', '#3C2F70', '#3C2F70', '#3C2F70', '#3C2F70']
      }
    },
    {
      id: 'teal', title: 'Teal', detail: 'A calm accent for long scans.',
      colors: {
        mark: '#BFEAE2', strong: '#0F8F7D', fill: '#E8F8F4', ink: '#0A6F61',
        series: ['#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D', '#0F8F7D'],
        seriesFill: ['#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2', '#BFEAE2']
      },
      dark: {
        mark: '#1B6E62', strong: '#2CC3AC', fill: '#0F2A27', ink: '#7FE3D3',
        series: ['#2CC3AC', '#2CC3AC', '#2CC3AC', '#2CC3AC', '#2CC3AC', '#2CC3AC'],
        seriesFill: ['#185C52', '#185C52', '#185C52', '#185C52', '#185C52', '#185C52']
      }
    },
    {
      id: 'amber', title: 'Amber', detail: 'A warmer signal for charts.',
      colors: {
        mark: '#F4D6AB', strong: '#C45C16', fill: '#FFF4E6', ink: '#8D3E0C',
        series: ['#C45C16', '#C45C16', '#C45C16', '#C45C16', '#C45C16', '#C45C16'],
        seriesFill: ['#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB', '#F4D6AB']
      },
      dark: {
        mark: '#8A5320', strong: '#E8913F', fill: '#2C1C0D', ink: '#F6C48C',
        series: ['#E8913F', '#E8913F', '#E8913F', '#E8913F', '#E8913F', '#E8913F'],
        seriesFill: ['#6E4319', '#6E4319', '#6E4319', '#6E4319', '#6E4319', '#6E4319']
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
      },
      dark: {
        mark: '#4A5462', strong: '#E9EDF3', fill: '#222834', ink: '#E9EDF3',
        series: ['#E9EDF3', '#D0D7E1', '#B0BAC8', '#8F9BAB', '#6F7C8D', '#535F6E'],
        seriesFill: ['#4A5462', '#414A57', '#39414D', '#313842', '#2A3039', '#232830']
      }
    },
    {
      id: 'ocean', title: 'Ocean', detail: 'One blue hue from deep to light.',
      colors: {
        mark: '#BDD2F4', strong: '#174EA6', fill: '#EEF4FF', ink: '#123D80',
        series: ['#174EA6', '#2A63B8', '#487AC5', '#6D93D2', '#91ACE0', '#B6C6EC'],
        seriesFill: ['#BDD2F4', '#C7D9F5', '#D2E0F7', '#DDE7F9', '#E8EFFB', '#F1F5FC']
      },
      dark: {
        mark: '#35507F', strong: '#8EB5F5', fill: '#16203A', ink: '#C3D4F5',
        series: ['#C3D4F5', '#A5BDEE', '#879FE0', '#6B87D2', '#526FBD', '#3C58A3'],
        seriesFill: ['#39507F', '#344872', '#2F4166', '#293A5A', '#24334E', '#1F2C42']
      }
    },
    {
      id: 'forest', title: 'Forest', detail: 'A measured green scale for distributions.',
      colors: {
        mark: '#BDE2CC', strong: '#1E6B45', fill: '#EBF7EF', ink: '#155334',
        series: ['#1E6B45', '#2E7D52', '#4B9369', '#6AA77F', '#8AB995', '#B1D2B9'],
        seriesFill: ['#BDE2CC', '#C8E6D2', '#D4EBDD', '#DFEFE4', '#EAF4ED', '#F2F8F3']
      },
      dark: {
        mark: '#2F5C43', strong: '#7CC096', fill: '#122419', ink: '#BFE3CC',
        series: ['#BFE3CC', '#9ED2B1', '#7CC096', '#5BAE7C', '#419663', '#327D4E'],
        seriesFill: ['#2E5D43', '#2A553D', '#264E37', '#224731', '#1E3F2C', '#1A3826']
      }
    },
    {
      id: 'ember', title: 'Ember', detail: 'A warm scale with a precise edge.',
      colors: {
        mark: '#F3C9A1', strong: '#A64716', fill: '#FFF1E6', ink: '#7D350D',
        series: ['#A64716', '#B95822', '#C96B33', '#D78452', '#E39F73', '#F0C1A0'],
        seriesFill: ['#F3C9A1', '#F5D1AE', '#F7DABD', '#F9E2CC', '#FBEADF', '#FDF3EC']
      },
      dark: {
        mark: '#6E4527', strong: '#E09763', fill: '#271A0F', ink: '#F4CBA6',
        series: ['#F4CBA6', '#EBB183', '#E09763', '#D27D47', '#C0652F', '#A9501E'],
        seriesFill: ['#6B4229', '#633C25', '#5B3721', '#53311D', '#4B2C19', '#432615']
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
      },
      dark: {
        mark: '#3A548C', strong: '#6E9BFF', fill: '#17233C', ink: '#A6C2FF',
        series: ['#6E9BFF', '#33C4AC', '#F0913F', '#A98BFF', '#F0718F', '#4FC461'],
        seriesFill: ['#2C4276', '#185C52', '#6E4319', '#3C2F70', '#6B2C3D', '#245C2E']
      }
    },
    {
      id: 'colorblind', title: 'Okabe–Ito', detail: 'Distinct blue, orange, green, and violet hues.',
      colors: {
        mark: '#8CC6E2', strong: '#0072B2', fill: '#E8F4FA', ink: '#005A8D',
        series: ['#0072B2', '#E69F00', '#009E73', '#CC79A7', '#D55E00', '#56B4E9'],
        seriesFill: ['#BFDDEE', '#F8DFAC', '#BCE8DA', '#E8C9DD', '#F2C4A5', '#C9EAF7']
      },
      dark: {
        mark: '#2C566E', strong: '#56B4E9', fill: '#10202A', ink: '#9BD6F2',
        series: ['#56B4E9', '#E69F00', '#009E73', '#CC79A7', '#D55E00', '#8FD3F4'],
        seriesFill: ['#27566E', '#6B4B00', '#004935', '#5E374E', '#632B00', '#3F6274']
      }
    },
    {
      id: 'sunset', title: 'Sunset', detail: 'A softer set of distinct warm hues.',
      colors: {
        mark: '#9CC7C8', strong: '#264653', fill: '#EDF5F4', ink: '#1E3A46',
        series: ['#264653', '#2A9D8F', '#E9C46A', '#F4A261', '#E76F51', '#8D5A97'],
        seriesFill: ['#BFD4D7', '#BFE4DF', '#F7E7B6', '#F9D4B3', '#F6C0B8', '#D9C5DD']
      },
      dark: {
        mark: '#2F5A63', strong: '#3EC2B1', fill: '#12242A', ink: '#9FD8DF',
        series: ['#5FA8B8', '#3EC2B1', '#EBC96F', '#F2A566', '#EE7F63', '#A97BB5'],
        seriesFill: ['#2D5058', '#1B5D54', '#6D5C29', '#71492A', '#6E3A2C', '#4E3856']
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

/* A palette keeps its identity across schemes and changes only its lightness: the same
   named choice stays the same choice when the room does. */
export function paletteColorsFor(palette: ChartPaletteDefinition, scheme: ColorScheme): ChartPaletteColors {
  return scheme === 'dark' ? palette.dark : palette.colors;
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

export function chartThemeCssVariables(preferences: ChartThemePreferences, scheme: ColorScheme = 'light'): Record<string, string> {
  const selected = paletteColorsFor(chartPaletteFor(preferences.mode, preferences.palettes[preferences.mode]), scheme);
  const variables: Record<string, string> = {
    '--chart-mark': selected.mark,
    '--chart-mark-strong': selected.strong,
    '--chart-mark-fill': selected.fill,
    '--chart-mark-ink': selected.ink
  };
  selected.series.forEach((color, index) => {
    variables[`--chart-series-${index + 1}`] = color;
    variables[`--chart-series-${index + 1}-fill`] = selected.seriesFill[index] ?? selected.mark;
  });
  return variables;
}
