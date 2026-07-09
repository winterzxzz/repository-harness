# Design Specification: Symphony CRM-UI Redesign

- **Date**: 2026-07-09
- **Topic**: CRM-Style Workspace UI Redesign for Symphony Web UI

---

## 1. Overview & Objectives

The goal is to redesign the Symphony Web UI into a highly visual, high-density CRM-style Workspace dashboard. This layout will replace the existing basic centered modal and flat column structure with a professional split-pane workspace typical of modern CRM tools (such as Attio or folk.app).

### Key Features:
- **Unified 3-Column Layout**: 
  1. Left sidebar for navigation and global status filters.
  2. Center main workspace for views (Kanban Board / Table View / History and Metrics).
  3. Right sliding Drawer panel for contextual task details.
- **View Switcher**: Tabs in the center workspace to toggle between Kanban Board, Table View, and Logs/Analytics.
- **Table-First Database View**: A high-density data table showing tasks, status badges, priority lanes, and action buttons.
- **Modern Kanban Cards**: Refined card visuals with priority badges, progress bar indicators for active runs, and dependency alerts.
- **Micro-Animations**: Smooth sliding transition for the right drawer panel and card hovers.

---

## 2. Layout & Component Architecture

### Left Sidebar (`sidebar.tsx`)
- Logo & title section at the top.
- Navigation section (Work Board, Guided Intake, Run Logs).
- Status filters with colored circular indicators and dynamic task counts.
- Relocated and cleaned up dependency graph.

### Center Workspace (`main.tsx` & `board.tsx`)
- Header with page title, active run badge, search box, and active action button.
- View Tabs (Kanban Board / Table View / Logs).
- View Containers:
  - **Kanban Board**: Existing `BoardGrid` styled with 3 rounded columns (Ready, In Progress, Blocked) showing cards.
  - **Table View**: A new `TableView` component showing a clean data table with hoverable rows. When a row is clicked, it opens the detail drawer.
  - **Logs / Run History**: Clean console-like history list.

### Right Drawer Panel (`detail.tsx`)
- Replacing the `TaskDetailOverlay` modal with a sliding right drawer.
- The drawer slides in from the right edge with a width of `380px` on desktop.
- On mobile, it overlays full screen.
- Smooth slide-in animation using Tailwind `transition-transform duration-300 ease-out-expo`.

---

## 3. Visual Styling & Design Tokens (`styles.css` & `tailwind.config.ts`)

- **Color Palette (OKLCH)**:
  - Dark background: `oklch(10% 0.015 250)` (deep charcoal-blue).
  - Cards & Drawer background: `oklch(13% 0.015 250)` (slightly lighter).
  - Borders: `oklch(20% 0.015 250)` (delicate dark borders).
  - Primary violet-blue accent: `oklch(62% 0.17 255)`.
- **Card Styling**:
  - Border radius: `8px`.
  - Hover states: `hover:-translate-y-0.5 hover:shadow-md transition-all duration-200 ease-out`.
  - Drop shadow: subtle dark shadow for floating elements.

---

## 4. Test & Verification Plan

- Run standard build validation `npm run build`.
- Verify all existing 22 Playwright tests in `tests/board.spec.ts` continue to pass.
- Add specific selectors (e.g. `data-testid="view-table"`, `data-testid="detail-drawer"`) to prevent regression.
- Manual verification of layout responsiveness across desktop and mobile views.
