# Audie Frontend Guidelines

This folder records frontend working rules for Audie. The reference style is
Photon Web's split between UI primitives, app components, and data hooks, adapted
to Audie's smaller Tauri app. Do not copy Photon-only dependencies such as
shadcn, Radix, CVA, or SWR into Audie without a separate architecture decision.

## Component Boundaries

- Keep shared primitives in `src/components/ui`: buttons, inputs, menus,
  dialogs, badges, switches, and other controls with no Audie business logic.
- Keep feature components near the screen that owns them. If a file grows past
  one readable responsibility, create a feature folder beside it, such as
  `src/components/Settings/model-config/`.
- Screen components should read hooks, coordinate the main user flow, and compose
  child components. They should not contain provider-specific request code,
  keychain reads, DOM queries, or large repeated JSX fragments.
- Prefer passing explicit props over importing feature state from deep children.
  A child should be understandable from its prop type.

## Side Effects

- Put Tauri `invoke` calls behind hooks or feature helpers unless the command is
  one tiny one-off action at the screen boundary.
- Validate external payloads with the existing Zod schemas before UI code relies
  on them.
- Keep keychain reads/writes and provider tests out of presentational components.
  UI components may render fields and fire callbacks; helpers perform the command
  calls.
- Mutation failures should surface through existing Audie feedback components
  such as `StatusMessage`, `InlineNotice`, or toast-like local state.

## Styling

- Use Audie's Tailwind v4 tokens from `src/styles/tokens`; do not introduce a new
  design system package for small refactors.
- Use `src/components/ui/Icon` and `IconButton` for icons before adding new SVGs.
- Use stable dimensions for buttons, lists, cards, and dialogs so async status
  text does not shift the layout.
- Keep arbitrary Tailwind values local to genuinely custom app surfaces; prefer
  existing spacing, color, and typography tokens for repeated controls.

## Quality Gates

- Run `pnpm typecheck`, `pnpm lint`, and `pnpm build` after frontend refactors.
- Remove unused imports and stale hook dependencies instead of suppressing lint.
- Keep refactor commits behavior-preserving unless the slice explicitly says
  otherwise.
