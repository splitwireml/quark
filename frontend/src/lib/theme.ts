/* Light or dark is a property of the room, not of the workspace: the default follows the
   operating system, and the two explicit choices exist for when the room disagrees with it. */
export type ThemePreference = 'system' | 'light' | 'dark';
export type ColorScheme = 'light' | 'dark';

export const themeStorageKey = 'quark.color-scheme';
export const themePreferences: readonly ThemePreference[] = ['system', 'light', 'dark'];

export function isThemePreference(value: unknown): value is ThemePreference {
  return typeof value === 'string' && themePreferences.includes(value as ThemePreference);
}

export function readThemePreference(raw: string | null): ThemePreference {
  return isThemePreference(raw) ? raw : 'system';
}

export function resolveScheme(preference: ThemePreference, prefersDark: boolean): ColorScheme {
  if (preference === 'system') return prefersDark ? 'dark' : 'light';
  return preference;
}
