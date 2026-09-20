## Component design

Strict rule: follow Atomic Design for all UI work. Keep reusable primitives in atoms, compose them into molecules and organisms, and keep page-level composition in templates; do not duplicate or bypass existing components.

## Motion and interaction character

Aim for interfaces that feel living and breathing: a control should grow out of what the user just did rather than appear beside it. Prefer transforming an existing element over adding a new one — a button that becomes the field it opens, a list that reveals its rest in place, an action that answers where it was invoked. Keep it restrained: one authored moment per interaction, short easing, no motion that delays the task, and always a `prefers-reduced-motion` path. Do this wherever the interaction allows it, never as decoration added on top.

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.
<!-- CODEGRAPH_END -->
