# Symphony Web UI Design Context

The Symphony Web UI is a dense local operational controller. It should feel
quiet, direct, and repeatable for daily engineering work.

## Design Principles

- Board first: Drafts, Active, Ready, and Done are the primary viewport signal.
- Dense but readable: show many tasks without compressing cards into unreadable
  strips.
- Stable controls: actions should not jump, resize, or change meaning while a
  run is active.
- Explicit state: internal states remain visible in cards and details without
  becoming separate primary board columns.
- Evidence forward: review, failure, validation, changed files, and raw artifact
  paths should stay easy to inspect.
- Local and utilitarian: avoid marketing layout, decorative hero treatment,
  generic card galleries, gradient decoration, or low-density dashboards.

## Layout Rules

- Desktop uses a board-first layout with all four buckets visible without
  horizontal page overflow.
- Mobile should show the board within the first viewport after summary controls.
- Board columns scroll internally for dense states.
- Task cards keep a stable readable minimum height and never create horizontal
  page overflow.
- Detail dialogs are bounded on desktop and mobile, with long values wrapping
  inside their containers.

## Component Rules

- Use local shadcn-style primitives for buttons, badges, cards, separators, and
  framed controls.
- Use icons for familiar operational actions such as start, refresh, close,
  delete, retry, warning, and success.
- Keep status tones consistent across cards, detail headers, review panels, and
  sidebar summaries.
- Do not introduce nested decorative cards or unrelated visual containers around
  the board.

## Typography And Density

- Compact panels use compact type; reserve larger type for the page title and
  major section headings.
- Long IDs, run IDs, paths, failure categories, and validation commands must
  wrap or truncate inside stable bounds.
- Uppercase labels are acceptable for metadata but should not dominate the
  primary task title or action text.

## State Polish

- Loading, error, disabled, success, active-run, review, and Needs Attention
  states should be visually deliberate.
- Disabled controls should explain unavailable actions through state and nearby
  context rather than appearing broken.
- Reduced-motion mode suppresses decorative animation.
- Confetti is limited to lightweight task-detail close feedback and must never
  obscure operational content.

## Review Evidence

Design polish review should use:

- Desktop board screenshot.
- Mobile board screenshot.
- Desktop detail screenshot.
- Mobile detail screenshot.
- Overflow checks for page, board, cards, and detail dialogs.
- Existing product contract in `docs/product/symphony-web-ui-controller.md`.
