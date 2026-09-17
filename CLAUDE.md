## Component design

Strict rule: follow Atomic Design for all UI work. Keep reusable primitives in atoms, compose them into molecules and organisms, and keep page-level composition in templates; do not duplicate or bypass existing components.

## Motion and interaction character

Aim for interfaces that feel living and breathing: a control should grow out of what the user just did rather than appear beside it. Prefer transforming an existing element over adding a new one — a button that becomes the field it opens, a list that reveals its rest in place, an action that answers where it was invoked. Keep it restrained: one authored moment per interaction, short easing, no motion that delays the task, and always a `prefers-reduced-motion` path. Do this wherever the interaction allows it, never as decoration added on top.
