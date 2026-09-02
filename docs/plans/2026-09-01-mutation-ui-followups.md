# Mutation UI follow-ups

Frontend-only, preserve the current dirty mutation work, no dependencies or commit.

1. Add tested `buildIfExpression` and `buildSwitchExpression` SQL helpers.
2. Reuse FormulaMenu for modify + rename; expose the name field and exclude only the current target from uniqueness checks.
3. Add a `FormulaButtonGroup` molecule and split helpers into Type, Arithmetic, Compare, Logic, and Conditional groups. `==` emits DuckDB `=`.
4. IF and Switch use one non-nestable structured builder with click-selectable operand fields; columns/helpers edit the selected operand.
5. Double-click a column label for inline rename with cell-editor-style focus, Enter, Escape, and blur behavior; reuse the same rename query path.
6. Run focused tests, Svelte autofix/check/build, backend tests, Docker rebuild, persistence check, and live UI smoke.
