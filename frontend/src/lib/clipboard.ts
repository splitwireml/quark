// A copied cell range travels as two flavours: tab-separated text for spreadsheets and
// plain editors, and an HTML table so mail clients and documents keep the grid.

// Spreadsheets read a tab or newline inside a value as a new cell unless the value is quoted.
function escapeCell(text: string): string {
  return /[\t\n\r"]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function escapeHtml(text: string): string {
  return text.replace(/[&<>]/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[character]!);
}

export function rangeToText(matrix: string[][]): string {
  return matrix.map((row) => row.map(escapeCell).join('\t')).join('\n');
}

export function rangeToHtml(matrix: string[][]): string {
  const rows = matrix
    .map((row) => `<tr>${row.map((cell) => `<td style="border:1px solid #d5d8de;padding:4px 8px">${escapeHtml(cell) || '&nbsp;'}</td>`).join('')}</tr>`)
    .join('');
  return `<table style="border-collapse:collapse;font-family:sans-serif;font-size:13px">${rows}</table>`;
}
