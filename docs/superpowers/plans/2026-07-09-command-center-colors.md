# Command Center Color Theme Customization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Customize the dark mode styles and color accents of the Symphony Web UI controller to match a dark-neutral palette with emerald green primary accents and custom dark violet decision states.

**Architecture:** Map tailwind configuration for `violet` to CSS variables, then modify the variables inside `styles.css` to update colors across the layout components (cards, board headers, metrics strip) automatically.

**Tech Stack:** Vite, React, Tailwind CSS

---

### Task 1: Update Tailwind Config

**Files:**
- Modify: `crates/harness-symphony/web-ui/tailwind.config.ts`

- [ ] **Step 1: Write code modifications to `tailwind.config.ts`**

Map the `violet` colors to CSS variables so that Tailwind classes like `bg-violet-950`, `border-violet-900`, etc., pull from CSS theme variables.

Replace the `colors` config in `crates/harness-symphony/web-ui/tailwind.config.ts`:
```ts
      colors: {
        border: "var(--border)",
        input: "var(--input)",
        ring: "var(--ring)",
        background: "var(--background)",
        foreground: "var(--foreground)",
        primary: {
          DEFAULT: "var(--primary)",
          foreground: "var(--primary-foreground)"
        },
        muted: {
          DEFAULT: "var(--muted)",
          foreground: "var(--muted-foreground)"
        },
        accent: {
          DEFAULT: "var(--accent)",
          foreground: "var(--accent-foreground)"
        },
        destructive: {
          DEFAULT: "var(--destructive)",
          foreground: "var(--destructive-foreground)"
        },
        warning: {
          DEFAULT: "var(--warning)"
        },
        card: {
          DEFAULT: "var(--card)",
          foreground: "var(--card-foreground)"
        },
        violet: {
          50: "var(--violet-50)",
          200: "var(--violet-200)",
          400: "var(--violet-400)",
          500: "var(--violet-500)",
          800: "var(--violet-800)",
          900: "var(--violet-900)",
          950: "var(--violet-950)"
        }
      }
```

- [ ] **Step 2: Commit Tailwind configuration changes**

Run:
```bash
git add crates/harness-symphony/web-ui/tailwind.config.ts
git commit -m "feat: map tailwind violet colors to CSS variables in config"
```
Expected output: Commit succeeds.

---

### Task 2: Define Theme Variables in CSS

**Files:**
- Modify: `crates/harness-symphony/web-ui/src/styles.css`

- [ ] **Step 1: Add violet variable overrides under `:root` (light mode fallback)**

Modify the `:root` block to map fallback variables for standard light mode:
```css
:root {
  --background: oklch(99% 0.005 250);
  --foreground: oklch(18% 0.015 250);
  --card: oklch(100% 0 0);
  --card-foreground: oklch(18% 0.015 250);
  --primary: oklch(52% 0.18 255);
  --primary-foreground: oklch(100% 0 0);
  --muted: oklch(96% 0.005 250);
  --muted-foreground: oklch(45% 0.015 250);
  --accent: oklch(92% 0.02 255);
  --accent-foreground: oklch(18% 0.015 250);
  --destructive: oklch(55% 0.18 20);
  --destructive-foreground: oklch(98% 0.01 20);
  --border: oklch(90% 0.01 250);
  --input: oklch(88% 0.01 250);
  --ring: oklch(52% 0.18 255);
  --warning: oklch(65% 0.16 50);
  
  --violet-50: oklch(97.5% 0.015 290);
  --violet-200: oklch(89.5% 0.05 290);
  --violet-400: oklch(75.5% 0.15 290);
  --violet-500: oklch(62% 0.22 290);
  --violet-800: oklch(42% 0.18 290);
  --violet-900: oklch(35% 0.16 290);
  --violet-950: oklch(25% 0.13 290);
}
```

- [ ] **Step 2: Update `.dark` variables with the target color scheme**

Modify the `.dark` class block in `crates/harness-symphony/web-ui/src/styles.css`:
```css
.dark {
  --background: oklch(9.3% 0.002 286);
  --foreground: oklch(97% 0.002 286);
  --card: oklch(12.7% 0.003 286);
  --card-foreground: oklch(95% 0.002 286);
  --primary: oklch(69.8% 0.176 162);
  --primary-foreground: oklch(100% 0 0);
  --muted: oklch(14.5% 0.004 286);
  --muted-foreground: oklch(70% 0.005 286);
  --accent: oklch(16% 0.004 286);
  --accent-foreground: oklch(90% 0.003 286);
  --destructive: oklch(50% 0.18 20);
  --destructive-foreground: oklch(98% 0.01 20);
  --border: oklch(18% 0.003 286);
  --input: oklch(12.7% 0.003 286);
  --ring: oklch(69.8% 0.176 162);
  --warning: oklch(70% 0.16 50);

  --violet-50: oklch(97% 0.01 300);
  --violet-200: oklch(89% 0.06 300);
  --violet-400: oklch(72% 0.16 300);
  --violet-500: oklch(60% 0.22 300);
  --violet-800: oklch(40% 0.18 300);
  --violet-900: oklch(14.8% 0.027 304);
  --violet-950: oklch(10.9% 0.016 304);
}
```

- [ ] **Step 3: Commit CSS theme variables modifications**

Run:
```bash
git add crates/harness-symphony/web-ui/src/styles.css
git commit -m "feat: customize dark theme background, primary accent, and violet colors"
```
Expected output: Commit succeeds.

---

### Task 3: Build & Verification

**Files:**
- None

- [ ] **Step 1: Verify web ui compiles successfully**

Run:
```bash
npm --prefix crates/harness-symphony/web-ui run build
```
Expected: command exits with `0` and compiles successfully.

- [ ] **Step 2: Run web UI E2E test suites**

Run:
```bash
npm --prefix crates/harness-symphony/web-ui run e2e
```
Expected: all Playwright browser tests pass.

- [ ] **Step 3: Run desktop smoke tests**

Run:
```bash
npm --prefix crates/harness-symphony/web-ui run desktop:smoke
```
Expected: smoke tests pass successfully.

- [ ] **Step 4: Run design-validation checks**

Run:
```bash
node .agents/skills/impeccable/scripts/detect.mjs --json crates/harness-symphony/web-ui/src crates/harness-symphony/web-ui/index.html
```
Expected: returns `[]` empty list of design violations.

- [ ] **Step 5: Run full cargo checks and workspace tests**

Run:
```bash
cargo fmt --check && cargo test --workspace --quiet && cargo clippy --workspace -- -D warnings
```
Expected: cargo checks pass successfully.
