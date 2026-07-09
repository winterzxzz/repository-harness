# Design Doc: Command Center Color Theme Customization

Date: 2026-07-09

## Source
- User prompt: request to change Symphony Web UI dark theme colors to match a specific screenshot.
- Screenshot: `/Users/winterzxzz/Library/Caches/com.apple.SwiftUI.Drag-FA1F4666-F2EF-454E-890C-8191EC07F40B/Screenshot 2026-07-09 at 15.36.58.png`

## Project Summary
We are updating the CSS variables in the dark theme of Symphony Web UI to match a dark-neutral palette with emerald green accents and custom dark-violet tones for the "Needs decision" / "Review" states, aligning with the screenshot.

## Target Gaps / Changes
1. **Background**: Main bg becomes `#0a0a0c` (`oklch(9.3% 0.002 286)`).
2. **Cards**: Card bg becomes `#131316` (`oklch(12.7% 0.003 286)`).
3. **Primary Accent (Emerald Green)**: Primary accent becomes `#10b981` (`oklch(69.8% 0.176 162)`).
4. **Violet colors**: Map Tailwind violet classes to customized variables under `.dark` so that:
   - Violet card background is `#130e1a` (`oklch(10.9% 0.016 304)`).
   - Violet card border is `#1b1226` (`oklch(14.8% 0.027 304)`).

## Affected Product Docs & Code Surfaces
- `crates/harness-symphony/web-ui/src/styles.css`
- `crates/harness-symphony/web-ui/tailwind.config.ts`

## Validation Shape
- Run `npm --prefix crates/harness-symphony/web-ui run build` to verify compilation.
- Run `npm --prefix crates/harness-symphony/web-ui run e2e` to verify E2E tests pass.
- Run `npm --prefix crates/harness-symphony/web-ui run desktop:smoke` to verify electron smoke tests.
- Run design-validation check: `node .agents/skills/impeccable/scripts/detect.mjs --json crates/harness-symphony/web-ui/src crates/harness-symphony/web-ui/index.html`.
