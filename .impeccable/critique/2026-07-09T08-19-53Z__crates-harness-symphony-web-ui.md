---
target: crates/harness-symphony/web-ui
total_score: 22
p0_count: 0
p1_count: 2
timestamp: 2026-07-09T08-19-53Z
slug: crates-harness-symphony-web-ui
---
⚠️ DEGRADED: single-context (spawn_agent unavailable in this session)

### Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | Good active run indicator, but status tones and transitions are basic. |
| 2 | Match System / Real World | 3 | Clear concepts ("Ready", "Blocked", "Review"), but some technical jargon leaks in error panels. |
| 3 | User Control and Freedom | 2 | Dialogs have Esc dismiss, but no undo for actions or quick keyboard shortcuts. |
| 4 | Consistency and Standards | 2 | Simple list layout in sidebar, card border styles vary slightly, button sizes are inconsistent. |
| 5 | Error Prevention | 3 | Confirmation dialogs are present, but run actions lack proactive safety guardrails. |
| 6 | Recognition Rather Than Recall | 2 | Dependency graph is a text list, hard to visualize flow without memorizing IDs. |
| 7 | Flexibility and Efficiency | 1 | No filtering presets, no bulk actions, no keyboard navigation. |
| 8 | Aesthetic and Minimalist Design | 2 | Neutral light theme feels like generic boilerplate (SaaS cliché). Lacks professional command-center depth and typography harmony. |
| 9 | Error Recovery | 3 | Recovery actions exist, but failure messages could be more actionable. |
| 10 | Help and Documentation | 1 | No inline tooltips or help panel for new users. |
| **Total** | | **22/40** | **Acceptable** |

### Anti-Patterns Verdict

**LLM Assessment**: The UI currently resembles a generic Tailwind template with standard SaaS-cliché styling. The card grid columns are identical and monotonous, the color palette is flat, and the overall spacing is somewhat cramped. It lacks the cohesive visual density and professional polish expected of an interactive developer control center (e.g., Linear-like styling, dark mode support, and refined micro-animations).

**Deterministic Scan**: The static detector scan returned no slop errors (`[]`), meaning the code is structurally clean but lacks advanced aesthetic styling, visual hierarchy, and state feedback.

**Visual Overlays**: No visual overlay is available since no browser automation is active in this session.

### Overall Impression

The Symphony Command Center is highly functional but feels like a basic wireframe. The single biggest opportunity is to redesign the layout into a sleek, premium, dark-mode-first command center with high-density visual hierarchy, modern typography (Inter/mono pairing), custom cards with subtle borders, and smooth state transitions.

### What's Working

1. **Board Layout Structure**: The six-column division matches the workflow states perfectly.
2. **Detail Modal Usability**: Keyboard focus trapping and Esc key listener are correctly implemented.
3. **Pulsing Active Run State**: The header badge clearly communicates active execution.

### Priority Issues

* **[P1] Flat Aesthetic & Lack of Dark Mode**
  * **Why it matters**: Developers spend hours looking at tools; a plain, bright light-only interface causes eye strain and feels cheap.
  * **Fix**: Introduce a premium dark/light mode system with custom CSS variables (OKLCH-based) and a sleek dark theme.
  * **Suggested command**: `$impeccable colorize`

* **[P1] Repetitive Task Card Grid & Low Density**
  * **Why it matters**: Task cards look identical and cluttered, making scanability difficult.
  * **Fix**: Restructure task cards with subtle color tones based on status, improved typography weights, and a cleaner footer.
  * **Suggested command**: `$impeccable layout`

* **[P2] Text-Based Dependency Graph**
  * **Why it matters**: The sidebar graph is a text list that doesn't visually convey the workflow order.
  * **Fix**: Redesign the dependency list into a mini visual tree or timeline with clear indicators.
  * **Suggested command**: `$impeccable layout`

* **[P2] Lack of Micro-Animations & Interactive Feedback**
  * **Why it matters**: Transitions feel abrupt, giving a static, unpolished vibe.
  * **Fix**: Add smooth transitions on hover, active state, and state changes (using ease-out-expo).
  * **Suggested command**: `$impeccable animate`

### Persona Red Flags

**Alex (Power User)**:
- No global keyboard shortcuts to search (`/`), refresh (`R`), or navigate columns.
- Starting a task requires multiple confirmation dialogs that slow down fast iteration.

**Jordan (First-Timer)**:
- "Proof: configured/neutral" and other technical states are confusing without tooltips.
- The distinction between "Ready" and "Blocked" cards is too subtle visually.

### Minor Observations

- The search input box has a static height that doesn't perfectly align with the refresh button.
- The font sizes in the summary cards are slightly large, reducing information density.

### Questions to Consider

- Should the Symphony Command Center default to a dark mode interface to match modern developer tool aesthetic?
- Can we streamline the confirmation dialogs into toast notifications or undo-actions?
