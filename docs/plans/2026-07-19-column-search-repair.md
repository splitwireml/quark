# Column search repair

**Files:** `frontend/src/App.svelte`, `frontend/src/app.css`

1. Keep the search input focused: remove programmatic header focus. Derive case-insensitive matches from the existing visible columns; an empty query returns no matches and changes no column visibility.
2. Show a compact adjacent match count. On input, reset to and scroll the first match. On Enter, advance a modulo index through matches and scroll that header into view.
3. Render matched name substrings in header labels with a small yellow bold `<mark>` treatment.

**Acceptance:** typing a full name preserves input focus and scrolls to that column; clearing leaves all columns visible; count is correct; Enter cycles each match; every matched substring is visibly highlighted.

**Checks:** `npm run check && npm run build`, then browser smoke: type multi-letter query, confirm focus/count/scroll; Enter twice; clear; inspect table still has all headers.
