# Keyboard commands

Use Cmd on macOS or Ctrl on Windows/Linux. An arrow between keys means a sequence: press the modified key, then press the next letter with or without Cmd/Ctrl held. A compact bottom-right hint shows available continuations and a three-second countdown. Each stage times out automatically. Escape, clicking elsewhere, entering a text field, or leaving the window cancels the sequence.

| Keys | Action |
| --- | --- |
| Cmd+A | Toggle the action wheel |
| Cmd+F → C | Find and select a column |
| Cmd+F → F | Find cell values |
| Cmd+E → F / S / H / P | Filter / sort / hide / pin the selected column |
| Cmd+B → B | Toggle the source sidebar |
| Cmd+B → T → number | Show numbered sources and switch to one |
| Cmd+Shift+V | Open Versions |
| Cmd+R | Refresh the current View |
| Cmd+S | Save pending changes as a Version (Stop recording) |
| Cmd+Z / Cmd+Shift+Z | Undo / redo |
| Cmd+, | Settings |
| ? | Search commands |

Settings → Action menu offers Simple (default) and Comprehensive, saved on this device. Simple exposes Aggregate (A), Join (J), Columns (C), and Dedupe (D), with small Row spacing (R) and Fit columns (W) controls. The four operations use an arrow-key layout: ↑ Aggregate above ← Join, ↓ Columns, and → Dedupe. Each arrow selects its matching operation; Enter opens it. Letter keys still open directly. Comprehensive keeps the full wheel including Find column and SQL. Cmd+F → C opens a compact column search field; matching results appear only after typing, arrows choose a result, Enter selects it, and Escape closes the finder. The existing Columns operation menu is unchanged. Arrows cycle the choices; Enter opens one. Each choice also displays a direct letter key. Escape returns to the wheel from its controls, then closes it. Operation menus keep the same size and toolbar placement as their buttons; hidden-toolbar menus anchor below the View header. Join keeps Sources accessible for choosing its inputs. Cmd+B starts a sequence without changing sidebar visibility; Cmd+B → T opens source selection even when the sidebar is closed.

Commands operate on the selected cell's column or highlighted column header. A command without a target opens the column picker first. Text fields and editors retain their normal select-all, typing, clipboard, and text undo behavior. A claimed command or sequence does not also edit a cell or invoke its browser default. Browser/OS-reserved commands are avoided; the same actions remain reachable through buttons.

Cell search runs on the server across all rows of the current filtered, sorted View and searches visible columns. It finds case-insensitive literal substrings, including numbers cast to text. Enter moves to the next matching cell, Shift+Enter to the previous, wrapping at the ends. Only the matching page is fetched.

Undo first reverses pending changes, then steps through saved Version parents. Redo retraces that exact path; new recorded changes clear redo. Arrow keys in Versions only move focus; Enter activates the Version. Existing pending-change discard confirmation still applies when switching Versions manually.

Settings → Appearance → Toolbar visibility supports Always show (default), Show on hover, and Always hide. Hover mode has a focusable reveal edge and stays open while a menu is active. The preference persists on the device; filters, sorts, and Version state remain visible in all modes. Actions in the title bar always provides access to the wheel.

## Verification

Run `npm --prefix frontend test`, `npm --prefix frontend run check`, and `.venv/bin/python -m pytest -q`. The command resolver tests cover scope, sequences, cancellation, modifiers, and IME composition. The server test covers search beyond page boundaries, wrapping in both directions, literal quotes/percent signs, numeric cells, filtered/sorted results, and invalid input.
