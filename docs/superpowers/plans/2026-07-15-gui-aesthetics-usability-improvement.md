# GUI Aesthetics & Usability Improvement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Tauri GUI (`crates/gui/frontend`) its own visual identity (emerald-green, friendly, Inter + lucide-react) and fix the five concrete usability gaps found in the wizard and main screen (silent errors, no progress indicator, no back navigation, raw hex/USB ids shown instead of friendly names) — plus a small backend addition so the main screen can show which monitor input is already active.

**Architecture:** Phase 1 lays the visual foundation (Tailwind CSS v4 + design tokens, self-hosted Inter, lucide-react icons, Vitest/RTL test tooling) and shared primitives (`InlineError`, `StatusBadge`, `ProgressBar`). Phase 2 reworks the wizard's state machine to support back navigation with preserved answers, and restyles each step. Phase 3 adds one read-only Tauri command (`current_input`) so the main screen can highlight the active input, then restyles the main screen. Phase 4 updates `MANUAL_TEST_GUI.md` and gates completion on cargo/npm builds and tests passing, with the actual hardware pass left for the user to run (this dev environment has no display/USB hardware).

**Tech Stack:** React 18 + TypeScript + Vite 5 (existing), Tailwind CSS v4 (`@tailwindcss/vite`), CSS Modules (existing Vite built-in support), `lucide-react`, `@fontsource-variable/inter`, Vitest + `@testing-library/react` + `jsdom` (new — this frontend currently has zero test tooling), Rust/`ddc-hi` (existing, one new trait method).

## Global Constraints

- Window stays fixed at 480×640 (`crates/gui/src-tauri/tauri.conf.json` — do not change `width`/`height`/`resizable`).
- Theme stays automatic via `prefers-color-scheme` only — no manual light/dark toggle UI.
- No JS animation library (no `framer-motion` etc.) — all transitions are CSS-only (`transition`/`@keyframes`), respecting `prefers-reduced-motion`.
- Primary color is Tailwind's built-in `emerald` scale (`emerald-600` `#059669` as the primary action color) — do not invent a separate custom palette.
- Typography is Inter, self-hosted via `@fontsource-variable/inter` (no CDN, no Google Fonts `<link>`).
- Icons are from `lucide-react` only.
- Styling is Tailwind utility classes for layout/spacing/color/typography; CSS Modules only for the three elaborate cases: wizard step transition, `ProgressBar` fill, `StatusBadge` pulse dot.
- USB vendor-name and VCP-input-name lookups are small curated tables with a raw-value fallback — no `usb-ids` crate, no bundling the full MCCS vendor database.
- Error display is inline (anchored to the failing action), never a toast/notification.
- Every new pure TS function and every new/changed React component gets a Vitest (+ React Testing Library where it renders) test. Rust changes follow the existing precedent in this codebase: pure parsing logic gets unit tests (see `crates/ddc-backend/src/capabilities.rs`), hardware-dependent DDC calls do not (see `ddchi_reader.rs`, which has none) — verified instead via `MANUAL_TEST_GUI.md`.
- `cargo build --workspace` and `cargo test --workspace` must stay green throughout; same for `npm run build` and the new `npm run test` in `crates/gui/frontend`.
- Final acceptance requires a real-hardware pass of the updated `MANUAL_TEST_GUI.md` (LG 34GL750 + DisplayLink dongle 17e9:6000 + Logitech MX Keys/Unifying, per `docs/DECISIONS.md`) — this must be run by the user; note it as the last, unchecked step.

---

## File Structure

New files:
- `crates/gui/frontend/vitest.setup.ts` — RTL cleanup + jest-dom matchers
- `crates/gui/frontend/src/vcpLabels.ts` (+ `.test.ts`) — VCP 0x60 code → friendly name
- `crates/gui/frontend/src/usbVendorLabels.ts` (+ `.test.ts`) — vendor id → friendly name
- `crates/gui/frontend/src/components/InlineError.tsx` (+ `.test.tsx`)
- `crates/gui/frontend/src/components/StatusBadge.tsx` + `.module.css` (+ `.test.tsx`)
- `crates/gui/frontend/src/components/ProgressBar.tsx` + `.module.css` (+ `.test.tsx`)
- `crates/gui/frontend/src/wizard/Wizard.module.css`
- `crates/gui/frontend/src/wizard/Wizard.test.tsx`
- `crates/gui/frontend/src/wizard/DeviceStep.test.tsx`
- `crates/gui/frontend/src/wizard/MonitorStep.test.tsx`
- `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx`
- `crates/gui/frontend/src/MainScreen.test.tsx`

Modified files:
- `crates/gui/frontend/package.json` — new deps + `test` script
- `crates/gui/frontend/vite.config.ts` — Tailwind plugin + Vitest config
- `crates/gui/frontend/src/main.tsx` — import Inter font
- `crates/gui/frontend/src/styles.css` — Tailwind import + design tokens, drop hand-rolled rules superseded by Tailwind
- `crates/gui/frontend/src/wizard/Wizard.tsx` — lifted state, back navigation, progress bar, step transition
- `crates/gui/frontend/src/wizard/DeviceStep.tsx` — error handling, back button, friendly vendor names, restyle
- `crates/gui/frontend/src/wizard/MonitorStep.tsx` — error handling, back button, restyle, pre-fill on back
- `crates/gui/frontend/src/wizard/InputMappingStep.tsx` — error handling, back button, friendly VCP labels, restyle, pre-fill on back
- `crates/gui/frontend/src/MainScreen.tsx` — restyle, active-input highlight, inline errors
- `crates/gui/frontend/src/api.ts` — add `currentInput`
- `crates/ddc-backend/src/lib.rs` — add `MonitorReader::current_input`
- `crates/ddc-backend/src/ddchi_reader.rs` — implement `current_input`
- `crates/gui/src-tauri/src/commands.rs` — add `current_input` Tauri command
- `crates/gui/src-tauri/src/main.rs` — register the new command
- `MANUAL_TEST_GUI.md` — new scenarios

---

### Task 1: Frontend test tooling (Vitest + React Testing Library)

**Files:**
- Modify: `crates/gui/frontend/package.json`
- Modify: `crates/gui/frontend/vite.config.ts`
- Create: `crates/gui/frontend/vitest.setup.ts`
- Create (temporary, deleted at end of this task): `crates/gui/frontend/src/smoke.test.tsx`

**Interfaces:**
- Produces: `npm run test` (in `crates/gui/frontend`) runs Vitest once (`vitest run`); every later task's test files rely on this working.

- [ ] **Step 1: Add the new devDependencies**

Edit `crates/gui/frontend/package.json`:

```json
{
  "name": "kvm-switch-gui-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest run"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "@tauri-apps/api": "^2.0.0",
    "lucide-react": "^0.468.0",
    "@fontsource-variable/inter": "^5.1.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.3",
    "vite": "^5.4.0",
    "tailwindcss": "^4.0.0",
    "@tailwindcss/vite": "^4.0.0",
    "vitest": "^2.1.8",
    "jsdom": "^25.0.1",
    "@testing-library/react": "^16.0.1",
    "@testing-library/jest-dom": "^6.6.3"
  }
}
```

(`tailwindcss`/`@tailwindcss/vite`/`lucide-react`/`@fontsource-variable/inter` are used starting Task 2 — added here so a single `npm install` covers the whole plan.)

Run: `cd crates/gui/frontend && npm install`
Expected: lockfile updates, no errors.

- [ ] **Step 2: Wire Vitest into the Vite config**

Edit `crates/gui/frontend/vite.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
  },
});
```

(`@tailwindcss/vite` is added to the `plugins` array in Task 2, not here, to keep this step scoped to test tooling.)

- [ ] **Step 3: Add the RTL setup file**

Create `crates/gui/frontend/vitest.setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});
```

- [ ] **Step 4: Write a throwaway smoke test to prove the pipeline works**

Create `crates/gui/frontend/src/smoke.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

describe("vitest + jsdom + RTL wiring", () => {
  it("renders into jsdom and can be queried", () => {
    render(<div>hello from vitest</div>);
    expect(screen.getByText("hello from vitest")).toBeInTheDocument();
  });
});
```

- [ ] **Step 5: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test`
Expected: `1 passed`, no errors.

- [ ] **Step 6: Delete the smoke test**

Delete `crates/gui/frontend/src/smoke.test.tsx` — it proved the wiring works; it isn't a real regression test, and every task from here on adds real ones.

- [ ] **Step 7: Commit**

```bash
git add crates/gui/frontend/package.json crates/gui/frontend/package-lock.json crates/gui/frontend/vite.config.ts crates/gui/frontend/vitest.setup.ts
git commit -m "test: add Vitest + React Testing Library to the frontend"
```

---

### Task 2: Tailwind CSS v4 + design tokens

**Files:**
- Modify: `crates/gui/frontend/vite.config.ts`
- Modify: `crates/gui/frontend/src/styles.css`

**Interfaces:**
- Produces: Tailwind utility classes (`bg-emerald-600`, `dark:`, etc.) usable in every component from Task 6 onward; CSS custom property `--color-primary` usable from plain CSS (the `.module.css` files in Tasks 7–9).

- [ ] **Step 1: Add the Tailwind Vite plugin**

Edit `crates/gui/frontend/vite.config.ts`:

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
  },
});
```

- [ ] **Step 2: Replace the global stylesheet with Tailwind + tokens**

Replace the full contents of `crates/gui/frontend/src/styles.css`:

```css
@import "tailwindcss";

/* Design tokens. Tailwind v4's `dark:` variant already follows
   `prefers-color-scheme` by default (no config needed) — matches the
   grilling decision to keep theming automatic, no manual toggle. */
@theme {
  --font-sans:
    "Inter Variable", -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --color-primary: var(--color-emerald-600);
  --color-primary-hover: var(--color-emerald-700);
}

html,
body,
#root {
  height: 100%;
}

body {
  @apply bg-neutral-50 text-neutral-900 dark:bg-neutral-950 dark:text-neutral-100;
  font-family: var(--font-sans);
}
```

This drops the old hand-rolled `h1`/`h2`/`ul`/`li`/`select`/`button` rules — every component touched in later tasks replaces them with Tailwind utility classes directly, so nothing relies on the old global element selectors after Task 13.

- [ ] **Step 3: Verify the build picks up Tailwind**

Run: `cd crates/gui/frontend && npm run build`
Expected: build succeeds; `dist/assets/*.css` contains compiled Tailwind output (spot-check: `grep -c "emerald" crates/gui/frontend/dist/assets/*.css` returns > 0 once any component uses an `emerald-*` class — harmless if 0 right now since no component uses Tailwind classes yet; the real check is that the build doesn't error and `@import "tailwindcss"` resolved).

- [ ] **Step 4: Commit**

```bash
git add crates/gui/frontend/vite.config.ts crates/gui/frontend/src/styles.css
git commit -m "style: add Tailwind CSS v4 and design tokens"
```

---

### Task 3: Self-hosted Inter font

**Files:**
- Modify: `crates/gui/frontend/src/main.tsx`

**Interfaces:**
- Consumes: `@fontsource-variable/inter` (added to `package.json` in Task 1), `--font-sans` token (Task 2).
- Produces: Inter renders as the app's font with no network request.

- [ ] **Step 1: Import the font before the stylesheet**

Edit `crates/gui/frontend/src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 2: Verify the font is bundled, not fetched remotely**

Run: `cd crates/gui/frontend && npm run build`
Expected: build succeeds; `dist/assets/` contains `.woff2` files from `@fontsource-variable/inter` (confirm with `ls crates/gui/frontend/dist/assets/*.woff2` — should list at least one file). This confirms the font is bundled into the app rather than requested from a CDN at runtime.

- [ ] **Step 3: Commit**

```bash
git add crates/gui/frontend/src/main.tsx
git commit -m "style: self-host Inter Variable via @fontsource-variable/inter"
```

---

### Task 4: VCP input-code → friendly label utility

**Files:**
- Create: `crates/gui/frontend/src/vcpLabels.ts`
- Test: `crates/gui/frontend/src/vcpLabels.test.ts`

**Interfaces:**
- Produces: `vcpInputLabel(code: number): string` — used by `InputMappingStep` (Task 11) and `MainScreen` (Task 13).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/vcpLabels.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { vcpInputLabel } from "./vcpLabels";

describe("vcpInputLabel", () => {
  it("maps standard MCCS VCP 0x60 codes to friendly names", () => {
    expect(vcpInputLabel(0x0f)).toBe("DisplayPort 1");
    expect(vcpInputLabel(0x10)).toBe("DisplayPort 2");
    expect(vcpInputLabel(0x11)).toBe("HDMI 1");
    expect(vcpInputLabel(0x12)).toBe("HDMI 2");
  });

  it("falls back to the raw hex code for values the spec doesn't define here", () => {
    expect(vcpInputLabel(0x99)).toBe("0x99");
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- vcpLabels`
Expected: FAIL — `Cannot find module './vcpLabels'`.

- [ ] **Step 3: Implement it**

Create `crates/gui/frontend/src/vcpLabels.ts`:

```ts
/** Friendly names for VCP feature 0x60 (Input Source Select)'s enumerated
 * values, per the VESA MCCS 3.x standard. Falls back to the raw hex code
 * for vendor-specific values the spec doesn't define — see the grilling
 * decision to keep this a small static table rather than parsing the full
 * MCCS database. */
const VCP_INPUT_LABELS: Record<number, string> = {
  0x01: "VGA 1",
  0x02: "VGA 2",
  0x03: "DVI 1",
  0x04: "DVI 2",
  0x05: "Composite video 1",
  0x06: "Composite video 2",
  0x07: "S-Video 1",
  0x08: "S-Video 2",
  0x09: "Tuner 1",
  0x0a: "Tuner 2",
  0x0b: "Tuner 3",
  0x0c: "Component video 1",
  0x0d: "Component video 2",
  0x0e: "Component video 3",
  0x0f: "DisplayPort 1",
  0x10: "DisplayPort 2",
  0x11: "HDMI 1",
  0x12: "HDMI 2",
};

export function vcpInputLabel(code: number): string {
  return VCP_INPUT_LABELS[code] ?? `0x${code.toString(16).toUpperCase()}`;
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- vcpLabels`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/vcpLabels.ts crates/gui/frontend/src/vcpLabels.test.ts
git commit -m "feat: add VCP input-code to friendly-label lookup"
```

---

### Task 5: USB vendor-id → friendly name utility

**Files:**
- Create: `crates/gui/frontend/src/usbVendorLabels.ts`
- Test: `crates/gui/frontend/src/usbVendorLabels.test.ts`

**Interfaces:**
- Produces: `usbDeviceLabel(id: string): string` — used by `DeviceStep` (Task 9).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/usbVendorLabels.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { usbDeviceLabel } from "./usbVendorLabels";

describe("usbDeviceLabel", () => {
  it("labels the Logitech vendor id used by MX Keys/Unifying", () => {
    expect(usbDeviceLabel("046d:c52b")).toBe("Logitech (046d:c52b)");
  });

  it("labels the DisplayLink vendor id used by this project's USB switch", () => {
    expect(usbDeviceLabel("17e9:6000")).toBe("DisplayLink (17e9:6000)");
  });

  it("is case-insensitive on the vendor id", () => {
    expect(usbDeviceLabel("046D:C52B")).toBe("Logitech (046D:C52B)");
  });

  it("falls back to the raw id for unknown vendors", () => {
    expect(usbDeviceLabel("ffff:0001")).toBe("ffff:0001");
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- usbVendorLabels`
Expected: FAIL — `Cannot find module './usbVendorLabels'`.

- [ ] **Step 3: Implement it**

Create `crates/gui/frontend/src/usbVendorLabels.ts`:

```ts
/** Curated, non-exhaustive vendor-id → name lookup, limited to devices this
 * app expects to see: Logitech (MX Keys / Unifying receivers) and
 * DisplayLink (the dongle validated as this project's "USB switch" in
 * docs/DECISIONS.md §2 — vendor 17e9, not a dedicated KVM switch chip).
 * Falls back to the raw `vendor:product` id for anything else — the
 * grilling decision was to avoid bundling the full `usb-ids` database for a
 * setup-screen label. */
const VENDOR_NAMES: Record<string, string> = {
  "046d": "Logitech",
  "17e9": "DisplayLink",
};

export function usbDeviceLabel(id: string): string {
  const vendor = id.split(":")[0]?.toLowerCase();
  const name = vendor ? VENDOR_NAMES[vendor] : undefined;
  return name ? `${name} (${id})` : id;
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- usbVendorLabels`
Expected: `4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/usbVendorLabels.ts crates/gui/frontend/src/usbVendorLabels.test.ts
git commit -m "feat: add USB vendor-id to friendly-label lookup"
```

---

### Task 6: `InlineError` and `StatusBadge` primitives

**Files:**
- Create: `crates/gui/frontend/src/components/InlineError.tsx`
- Test: `crates/gui/frontend/src/components/InlineError.test.tsx`
- Create: `crates/gui/frontend/src/components/StatusBadge.tsx`
- Create: `crates/gui/frontend/src/components/StatusBadge.module.css`
- Test: `crates/gui/frontend/src/components/StatusBadge.test.tsx`

**Interfaces:**
- Produces: `InlineError({ message: string | null })` — used by every wizard step (Tasks 9–11), `Wizard` (Task 8), and `MainScreen` (Task 13).
- Produces: `StatusBadge({ status: "connected" | "disconnected" | "unknown" })` — used by `MainScreen` (Task 13).

- [ ] **Step 1: Write the failing InlineError test**

Create `crates/gui/frontend/src/components/InlineError.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { InlineError } from "./InlineError";

describe("InlineError", () => {
  it("renders nothing when message is null", () => {
    render(<InlineError message={null} />);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders the message with an alert role when present", () => {
    render(<InlineError message="failed to switch input" />);
    expect(screen.getByRole("alert")).toHaveTextContent("failed to switch input");
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- InlineError`
Expected: FAIL — `Cannot find module './InlineError'`.

- [ ] **Step 3: Implement InlineError**

Create `crates/gui/frontend/src/components/InlineError.tsx`:

```tsx
import { AlertCircle } from "lucide-react";

export function InlineError({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-md border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-800 dark:border-red-900/60 dark:bg-red-950 dark:text-red-200"
    >
      <AlertCircle size={16} className="mt-0.5 shrink-0" />
      <span>{message}</span>
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- InlineError`
Expected: `2 passed`.

- [ ] **Step 5: Write the failing StatusBadge test**

Create `crates/gui/frontend/src/components/StatusBadge.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBadge } from "./StatusBadge";

describe("StatusBadge", () => {
  it("shows Connected for status=connected", () => {
    render(<StatusBadge status="connected" />);
    expect(screen.getByText("Connected")).toBeInTheDocument();
  });

  it("shows Not connected for status=disconnected", () => {
    render(<StatusBadge status="disconnected" />);
    expect(screen.getByText("Not connected")).toBeInTheDocument();
  });

  it("shows Unknown for status=unknown", () => {
    render(<StatusBadge status="unknown" />);
    expect(screen.getByText("Unknown")).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- StatusBadge`
Expected: FAIL — `Cannot find module './StatusBadge'`.

- [ ] **Step 7: Implement StatusBadge**

Create `crates/gui/frontend/src/components/StatusBadge.module.css`:

```css
.badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 0.8125rem;
  font-weight: 500;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.connected .dot {
  background: var(--color-primary, #059669);
  animation: pulse 2s ease-in-out infinite;
}

.disconnected .dot {
  background: #9ca3af;
}

.unknown .dot {
  background: #d1d5db;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

@media (prefers-reduced-motion: reduce) {
  .connected .dot {
    animation: none;
  }
}
```

Create `crates/gui/frontend/src/components/StatusBadge.tsx`:

```tsx
import styles from "./StatusBadge.module.css";

type Status = "connected" | "disconnected" | "unknown";

const LABELS: Record<Status, string> = {
  connected: "Connected",
  disconnected: "Not connected",
  unknown: "Unknown",
};

export function StatusBadge({ status }: { status: Status }) {
  return (
    <span className={`${styles.badge} ${styles[status]}`}>
      <span className={styles.dot} />
      {LABELS[status]}
    </span>
  );
}
```

- [ ] **Step 8: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- StatusBadge`
Expected: `3 passed`.

- [ ] **Step 9: Commit**

```bash
git add crates/gui/frontend/src/components/InlineError.tsx crates/gui/frontend/src/components/InlineError.test.tsx crates/gui/frontend/src/components/StatusBadge.tsx crates/gui/frontend/src/components/StatusBadge.module.css crates/gui/frontend/src/components/StatusBadge.test.tsx
git commit -m "feat: add InlineError and StatusBadge shared components"
```

---

### Task 7: `ProgressBar` primitive

**Files:**
- Create: `crates/gui/frontend/src/components/ProgressBar.tsx`
- Create: `crates/gui/frontend/src/components/ProgressBar.module.css`
- Test: `crates/gui/frontend/src/components/ProgressBar.test.tsx`

**Interfaces:**
- Produces: `ProgressBar({ step: number, total: number })`, rendering `role="progressbar"` with `aria-valuenow` as a whole-number percentage — used by `Wizard` (Task 8).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/components/ProgressBar.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ProgressBar } from "./ProgressBar";

describe("ProgressBar", () => {
  it("reports 25% for step 1 of 4", () => {
    render(<ProgressBar step={1} total={4} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
  });

  it("reports 100% for step 4 of 4", () => {
    render(<ProgressBar step={4} total={4} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "100");
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- ProgressBar`
Expected: FAIL — `Cannot find module './ProgressBar'`.

- [ ] **Step 3: Implement it**

Create `crates/gui/frontend/src/components/ProgressBar.module.css`:

```css
.track {
  width: 100%;
  height: 4px;
  border-radius: 2px;
  background: rgb(0 0 0 / 8%);
  overflow: hidden;
}

@media (prefers-color-scheme: dark) {
  .track {
    background: rgb(255 255 255 / 12%);
  }
}

.fill {
  height: 100%;
  background: var(--color-primary, #059669);
  transition: width 250ms ease-out;
}

@media (prefers-reduced-motion: reduce) {
  .fill {
    transition: none;
  }
}
```

Create `crates/gui/frontend/src/components/ProgressBar.tsx`:

```tsx
import styles from "./ProgressBar.module.css";

export function ProgressBar({ step, total }: { step: number; total: number }) {
  const pct = Math.round((step / total) * 100);
  return (
    <div
      className={styles.track}
      role="progressbar"
      aria-valuenow={pct}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className={styles.fill} style={{ width: `${pct}%` }} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- ProgressBar`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/components/ProgressBar.tsx crates/gui/frontend/src/components/ProgressBar.module.css crates/gui/frontend/src/components/ProgressBar.test.tsx
git commit -m "feat: add ProgressBar shared component"
```

---

### Task 8: `Wizard.tsx` — lifted state, back navigation, progress bar

**Files:**
- Modify: `crates/gui/frontend/src/wizard/Wizard.tsx`
- Test: `crates/gui/frontend/src/wizard/Wizard.test.tsx`

**Interfaces:**
- Consumes: `ProgressBar` (Task 7), `InlineError` (Task 6), `saveConfig`/`Configuration`/`MonitorInfo` from `../api`.
- Produces: `Wizard`'s four child steps now receive `onBack?: () => void` (absent only on the very first step, which has nothing to go back to), `MonitorStep` receives `initialSelection?: MonitorInfo | null`, `InputMappingStep` receives `initialOnConnect?: number | null` / `initialOnDisconnect?: number | null`. This is the exact prop contract Tasks 10–12 must implement against.

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/wizard/Wizard.test.tsx`. This mocks the four step components (Wizard's job here is orchestration, not what each step renders — those get their own tests in Tasks 10–12) and `saveConfig`:

```tsx
import { describe, it, expect, vi, waitFor } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Wizard } from "./Wizard";

vi.mock("./DeviceStep", () => ({
  DeviceStep: ({ label, onSelected, onSkip, onBack }: any) => (
    <div>
      <p>{label}</p>
      <button onClick={() => onSelected("aaaa:bbbb")}>select-device</button>
      {onSkip && <button onClick={onSkip}>skip-device</button>}
      {onBack && <button onClick={onBack}>back-device</button>}
    </div>
  ),
}));

vi.mock("./MonitorStep", () => ({
  MonitorStep: ({ onSelected, onBack }: any) => (
    <div>
      <button
        onClick={() => onSelected({ display_index: 0, id: "mon-1", model_name: "Test Monitor" })}
      >
        select-monitor
      </button>
      <button onClick={onBack}>back-monitor</button>
    </div>
  ),
}));

vi.mock("./InputMappingStep", () => ({
  InputMappingStep: ({ onComplete, onBack }: any) => (
    <div>
      <button onClick={() => onComplete({ onConnect: 0x11, onDisconnect: null })}>finish</button>
      <button onClick={onBack}>back-inputs</button>
    </div>
  ),
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, saveConfig: vi.fn().mockResolvedValue(undefined) };
});

describe("Wizard", () => {
  it("shows 25% progress on the first step, with no back button", () => {
    render(<Wizard onComplete={() => {}} />);
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
    expect(screen.queryByText("back-device")).not.toBeInTheDocument();
  });

  it("preserves the monitor selection when navigating back to it", () => {
    render(<Wizard onComplete={() => {}} />);
    fireEvent.click(screen.getByText("select-device")); // -> mxkeys step, 50%
    fireEvent.click(screen.getByText("skip-device")); // -> monitor step, 75%
    fireEvent.click(screen.getByText("select-monitor")); // -> inputs step, 100%
    fireEvent.click(screen.getByText("back-inputs")); // -> back to monitor step
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "75");
  });

  it("calls saveConfig and onComplete with the assembled configuration", async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText("select-device"));
    fireEvent.click(screen.getByText("skip-device"));
    fireEvent.click(screen.getByText("select-monitor"));
    fireEvent.click(screen.getByText("finish"));

    await waitFor(() =>
      expect(onComplete).toHaveBeenCalledWith(
        expect.objectContaining({
          usb_device: "aaaa:bbbb",
          mxkeys_usb_device: null,
          on_usb_connect: "0x11",
          on_usb_disconnect: null,
          display_index: 0,
        }),
      ),
    );
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- Wizard.test`
Expected: FAIL — current `Wizard.tsx` doesn't render a `progressbar` role and has no back navigation.

- [ ] **Step 3: Implement it**

Create `crates/gui/frontend/src/wizard/Wizard.module.css`:

```css
.stepTransition {
  animation: fadeSlideIn 200ms ease-out;
}

@keyframes fadeSlideIn {
  from {
    opacity: 0;
    transform: translateX(8px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .stepTransition {
    animation: none;
  }
}
```

Replace `crates/gui/frontend/src/wizard/Wizard.tsx`:

```tsx
import { useState } from "react";
import { Configuration, MonitorInfo, saveConfig } from "../api";
import { DeviceStep } from "./DeviceStep";
import { MonitorStep } from "./MonitorStep";
import { InputMappingStep } from "./InputMappingStep";
import { ProgressBar } from "../components/ProgressBar";
import { InlineError } from "../components/InlineError";
import styles from "./Wizard.module.css";

interface WizardAnswers {
  switchDevice: string | null;
  mxkeysDevice: string | null;
  monitor: MonitorInfo | null;
  onConnect: number | null;
  onDisconnect: number | null;
}

const STEP_COUNT = 4;

const emptyAnswers: WizardAnswers = {
  switchDevice: null,
  mxkeysDevice: null,
  monitor: null,
  onConnect: null,
  onDisconnect: null,
};

export function Wizard({ onComplete }: { onComplete: (config: Configuration) => void }) {
  const [stepIndex, setStepIndex] = useState(0);
  const [answers, setAnswers] = useState<WizardAnswers>(emptyAnswers);
  const [saveError, setSaveError] = useState<string | null>(null);

  const goBack = () => setStepIndex((i) => Math.max(0, i - 1));

  const finish = async (onConnect: number, onDisconnect: number | null, monitor: MonitorInfo) => {
    const config: Configuration = {
      usb_device: answers.switchDevice!,
      mxkeys_usb_device: answers.mxkeysDevice || null,
      on_usb_connect: `0x${onConnect.toString(16)}`,
      on_usb_disconnect: onDisconnect !== null ? `0x${onDisconnect.toString(16)}` : null,
      on_usb_connect_source_addr: null,
      on_usb_connect_vcp_code: null,
      display_index: monitor.display_index,
    };
    try {
      setSaveError(null);
      await saveConfig(config);
      onComplete(config);
    } catch (err) {
      setSaveError(String(err));
    }
  };

  return (
    <div className="mx-auto flex h-full max-w-md flex-col gap-4 p-5">
      <ProgressBar step={stepIndex + 1} total={STEP_COUNT} />
      <div key={stepIndex} className={styles.stepTransition}>
        {stepIndex === 0 && (
          <DeviceStep
            key="switch-device"
            label="Select the KVM switch USB device"
            onSelected={(id) => {
              setAnswers((a) => ({ ...a, switchDevice: id }));
              setStepIndex(1);
            }}
          />
        )}
        {stepIndex === 1 && (
          <DeviceStep
            key="mxkeys-device"
            label="Select the MX Keys receiver (optional — plug it in, or skip)"
            onSelected={(id) => {
              setAnswers((a) => ({ ...a, mxkeysDevice: id }));
              setStepIndex(2);
            }}
            onSkip={() => {
              setAnswers((a) => ({ ...a, mxkeysDevice: "" }));
              setStepIndex(2);
            }}
            onBack={goBack}
          />
        )}
        {stepIndex === 2 && (
          <MonitorStep
            initialSelection={answers.monitor}
            onSelected={(monitor) => {
              setAnswers((a) => ({ ...a, monitor }));
              setStepIndex(3);
            }}
            onBack={goBack}
          />
        )}
        {stepIndex === 3 && answers.monitor && (
          <InputMappingStep
            displayIndex={answers.monitor.display_index}
            initialOnConnect={answers.onConnect}
            initialOnDisconnect={answers.onDisconnect}
            onBack={goBack}
            onComplete={({ onConnect, onDisconnect }) => {
              setAnswers((a) => ({ ...a, onConnect, onDisconnect }));
              finish(onConnect, onDisconnect, answers.monitor!);
            }}
          />
        )}
      </div>
      <InlineError message={saveError} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- Wizard.test`
Expected: `3 passed`.

Note: this test file will only fully pass once Tasks 9–11 update the real `DeviceStep`/`MonitorStep`/`InputMappingStep` signatures to accept `onBack`/`initialSelection`/`initialOnConnect`/`initialOnDisconnect` — but since this test mocks those modules entirely, it is self-contained and passes as soon as `Wizard.tsx` itself is correct, regardless of Tasks 9–11 having landed yet. `npm run build`'s `tsc` step, however, will show type errors against the real (not-yet-updated) step components until Tasks 9–11 land — expected and resolved by the end of Task 11.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/wizard/Wizard.tsx crates/gui/frontend/src/wizard/Wizard.module.css crates/gui/frontend/src/wizard/Wizard.test.tsx
git commit -m "feat: lift wizard state for back navigation and add progress bar"
```

---

### Task 9: `DeviceStep.tsx` — error handling, back button, friendly names, restyle

**Files:**
- Modify: `crates/gui/frontend/src/wizard/DeviceStep.tsx`
- Test: `crates/gui/frontend/src/wizard/DeviceStep.test.tsx`

**Interfaces:**
- Consumes: `usbDeviceLabel` (Task 5), `InlineError` (Task 6).
- Produces: `DeviceStep` now accepts `onBack?: () => void` (matches `Wizard`'s Task 8 contract).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/wizard/DeviceStep.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { DeviceStep } from "./DeviceStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("DeviceStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows an inline error when the initial snapshot fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("USB enumeration failed");
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    fireEvent.click(screen.getByText("Start"));

    expect(await screen.findByRole("alert")).toHaveTextContent("USB enumeration failed");
  });

  it("shows friendly vendor names for detected candidates", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]).mockResolvedValueOnce(["046d:c52b"]);
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);

    fireEvent.click(screen.getByText("Start"));
    await waitFor(() => screen.getByText("I plugged it in"));
    fireEvent.click(screen.getByText("I plugged it in"));

    expect(await screen.findByText("Logitech (046d:c52b)")).toBeInTheDocument();
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<DeviceStep label="Pick a device" onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });

  it("renders no back button when onBack is not provided", () => {
    render(<DeviceStep label="Pick a device" onSelected={() => {}} />);
    expect(screen.queryByLabelText("Back")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- DeviceStep`
Expected: FAIL — no error handling, no back button, no friendly-name rendering exist yet.

- [ ] **Step 3: Implement it**

Replace `crates/gui/frontend/src/wizard/DeviceStep.tsx`:

```tsx
import { useState } from "react";
import { ChevronLeft, Usb } from "lucide-react";
import { listUsbDevices } from "../api";
import { usbDeviceLabel } from "../usbVendorLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
  onSkip?: () => void;
  onBack?: () => void;
}

/** "Plug it in, click the one that appeared": snapshots the USB device list,
 * asks the user to plug in the device, re-snapshots, and highlights whatever
 * is new. */
export function DeviceStep({ label, onSelected, onSkip, onBack }: Props) {
  const [before, setBefore] = useState<string[] | null>(null);
  const [candidates, setCandidates] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const snapshotBefore = async () => {
    setError(null);
    try {
      setBefore(await listUsbDevices());
    } catch (err) {
      setError(String(err));
    }
  };

  const detectNew = async () => {
    setError(null);
    try {
      const after = await listUsbDevices();
      const beforeSet = new Set(before ?? []);
      setCandidates(after.filter((id) => !beforeSet.has(id)));
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        {onBack && (
          <button
            onClick={onBack}
            aria-label="Back"
            className="rounded-md p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <ChevronLeft size={18} />
          </button>
        )}
        <h2 className="flex items-center gap-2 text-base font-semibold text-neutral-900 dark:text-neutral-100">
          <Usb size={18} className="text-emerald-600" />
          {label}
        </h2>
      </div>

      {before === null && (
        <div className="flex gap-2">
          <button
            onClick={snapshotBefore}
            className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
          >
            Start
          </button>
          {onSkip && (
            <button
              onClick={onSkip}
              className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
            >
              Skip
            </button>
          )}
        </div>
      )}

      {before !== null && candidates.length === 0 && (
        <div className="flex flex-col gap-2">
          <p className="text-sm text-neutral-600 dark:text-neutral-400">
            Now plug in the device (or unplug/replug it).
          </p>
          <div className="flex gap-2">
            <button
              onClick={detectNew}
              className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
            >
              I plugged it in
            </button>
            {onSkip && (
              <button
                onClick={onSkip}
                className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
              >
                Skip
              </button>
            )}
          </div>
        </div>
      )}

      {candidates.length > 0 && (
        <ul className="flex flex-col gap-2">
          {candidates.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{usbDeviceLabel(id)}</span>
              <button
                onClick={() => onSelected(id)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
              >
                Use this
              </button>
            </li>
          ))}
        </ul>
      )}

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- DeviceStep`
Expected: `4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/wizard/DeviceStep.tsx crates/gui/frontend/src/wizard/DeviceStep.test.tsx
git commit -m "feat: add error handling, back nav, and friendly names to DeviceStep"
```

---

### Task 10: `MonitorStep.tsx` — error handling, back button, restyle

**Files:**
- Modify: `crates/gui/frontend/src/wizard/MonitorStep.tsx`
- Test: `crates/gui/frontend/src/wizard/MonitorStep.test.tsx`

**Interfaces:**
- Consumes: `InlineError` (Task 6).
- Produces: `MonitorStep` now accepts `initialSelection?: MonitorInfo | null` and `onBack?: () => void` (matches `Wizard`'s Task 8 contract).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/wizard/MonitorStep.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { MonitorStep } from "./MonitorStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const monitors = [
  { display_index: 0, id: "mon-a", model_name: "LG 34GL750 (A)" },
  { display_index: 1, id: "mon-b", model_name: "LG 34GL750 (B)" },
];

describe("MonitorStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows an inline error when monitor detection fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("no DDC displays found");
    render(<MonitorStep onSelected={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("no DDC displays found");
  });

  it("lists detected monitors and calls onSelected", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(monitors);
    const onSelected = vi.fn();
    render(<MonitorStep onSelected={onSelected} />);

    fireEvent.click((await screen.findAllByText("Use this monitor"))[0]);
    expect(onSelected).toHaveBeenCalledWith(monitors[0]);
  });

  it("marks the previously-selected monitor when navigating back to this step", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(monitors);
    render(<MonitorStep initialSelection={monitors[1]} onSelected={() => {}} />);

    const items = await screen.findAllByRole("listitem");
    expect(items[1]).toHaveTextContent("LG 34GL750 (B)");
    expect(items[1].querySelector("svg")).not.toBeNull(); // check icon marks the previous pick
    expect(items[0].querySelector("svg")).toBeNull();
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<MonitorStep onSelected={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- MonitorStep`
Expected: FAIL — no error handling, no back button, no `initialSelection` marking exist yet.

- [ ] **Step 3: Implement it**

Replace `crates/gui/frontend/src/wizard/MonitorStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ChevronLeft, Check, Monitor as MonitorIcon } from "lucide-react";
import { listMonitors, MonitorInfo } from "../api";
import { InlineError } from "../components/InlineError";

interface Props {
  initialSelection?: MonitorInfo | null;
  onSelected: (monitor: MonitorInfo) => void;
  onBack?: () => void;
}

export function MonitorStep({ initialSelection, onSelected, onBack }: Props) {
  const [monitors, setMonitors] = useState<MonitorInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listMonitors()
      .then(setMonitors)
      .catch((err) => setError(String(err)));
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        {onBack && (
          <button
            onClick={onBack}
            aria-label="Back"
            className="rounded-md p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <ChevronLeft size={18} />
          </button>
        )}
        <h2 className="flex items-center gap-2 text-base font-semibold text-neutral-900 dark:text-neutral-100">
          <MonitorIcon size={18} className="text-emerald-600" />
          Select the monitor this KVM setup controls
        </h2>
      </div>

      {monitors === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Detecting monitors…</p>
      )}
      {monitors !== null && monitors.length === 0 && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          No DDC-compatible monitors detected.
        </p>
      )}
      {monitors !== null && monitors.length > 0 && (
        <ul className="flex flex-col gap-2">
          {monitors.map((m) => {
            const isPrevious = initialSelection?.display_index === m.display_index;
            return (
              <li
                key={m.display_index}
                className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
              >
                <span className="flex items-center gap-2 text-sm">
                  {isPrevious && <Check size={14} className="text-emerald-600" />}
                  {m.model_name ?? m.id} (display index {m.display_index})
                </span>
                <button
                  onClick={() => onSelected(m)}
                  className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
                >
                  Use this monitor
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- MonitorStep`
Expected: `4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/frontend/src/wizard/MonitorStep.tsx crates/gui/frontend/src/wizard/MonitorStep.test.tsx
git commit -m "feat: add error handling, back nav, and restyle to MonitorStep"
```

---

### Task 11: `InputMappingStep.tsx` — error handling, back button, friendly VCP labels, restyle

**Files:**
- Modify: `crates/gui/frontend/src/wizard/InputMappingStep.tsx`
- Test: `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx`

**Interfaces:**
- Consumes: `vcpInputLabel` (Task 4), `InlineError` (Task 6).
- Produces: `InputMappingStep` now accepts `initialOnConnect?: number | null`, `initialOnDisconnect?: number | null`, `onBack?: () => void` (matches `Wizard`'s Task 8 contract).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/wizard/InputMappingStep.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { InputMappingStep } from "./InputMappingStep";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("InputMappingStep", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows an inline error when reading inputs fails", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("failed to read capabilities");
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("failed to read capabilities");
  });

  it("shows friendly labels instead of raw hex", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} />);
    expect(await screen.findByText("DisplayPort 1")).toBeInTheDocument();
    expect(screen.getByText("HDMI 1")).toBeInTheDocument();
  });

  it("pre-fills the previous selections when navigating back to this step", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]);
    render(
      <InputMappingStep
        displayIndex={0}
        initialOnConnect={0x11}
        initialOnDisconnect={0x0f}
        onComplete={() => {}}
      />,
    );
    const selects = await screen.findAllByRole("combobox");
    expect((selects[0] as HTMLSelectElement).value).toBe("17"); // 0x11 == 17
    expect((selects[1] as HTMLSelectElement).value).toBe("15"); // 0x0f == 15
  });

  it("calls onBack when the back button is clicked", () => {
    const onBack = vi.fn();
    render(<InputMappingStep displayIndex={0} onComplete={() => {}} onBack={onBack} />);
    fireEvent.click(screen.getByLabelText("Back"));
    expect(onBack).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- InputMappingStep`
Expected: FAIL — raw hex is shown instead of friendly labels, no error handling, no back button, no pre-fill.

- [ ] **Step 3: Implement it**

Replace `crates/gui/frontend/src/wizard/InputMappingStep.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { listInputs } from "../api";
import { vcpInputLabel } from "../vcpLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  displayIndex: number;
  initialOnConnect?: number | null;
  initialOnDisconnect?: number | null;
  onBack?: () => void;
  onComplete: (mapping: { onConnect: number; onDisconnect: number | null }) => void;
}

export function InputMappingStep({
  displayIndex,
  initialOnConnect = null,
  initialOnDisconnect = null,
  onBack,
  onComplete,
}: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(initialOnConnect);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(initialOnDisconnect);

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        {onBack && (
          <button
            onClick={onBack}
            aria-label="Back"
            className="rounded-md p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <ChevronLeft size={18} />
          </button>
        )}
        <h2 className="text-base font-semibold text-neutral-900 dark:text-neutral-100">Map inputs</h2>
      </div>

      {inputs === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Reading supported inputs…</p>
      )}

      {inputs !== null && (
        <>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input when the KVM switch connects to this host:
            <select
              value={onConnect ?? ""}
              onChange={(e) => setOnConnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">Select…</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input on disconnect (optional):
            <select
              value={onDisconnect ?? ""}
              onChange={(e) => setOnDisconnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">None</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>
          <button
            disabled={onConnect === null}
            onClick={() => onComplete({ onConnect: onConnect!, onDisconnect })}
            className="self-start rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Finish
          </button>
        </>
      )}

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- InputMappingStep`
Expected: `4 passed`.

- [ ] **Step 5: Run the whole frontend test suite and the build**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all tests pass; `tsc` now type-checks cleanly since `Wizard.tsx` (Task 8) and all three step components (Tasks 9–11) agree on the prop contract.

- [ ] **Step 6: Commit**

```bash
git add crates/gui/frontend/src/wizard/InputMappingStep.tsx crates/gui/frontend/src/wizard/InputMappingStep.test.tsx
git commit -m "feat: add error handling, back nav, and friendly VCP labels to InputMappingStep"
```

---

### Task 12: Backend — `current_input` command

**Files:**
- Modify: `crates/ddc-backend/src/lib.rs:27-30`
- Modify: `crates/ddc-backend/src/ddchi_reader.rs`
- Modify: `crates/gui/src-tauri/src/commands.rs`
- Modify: `crates/gui/src-tauri/src/main.rs:320-327`
- Modify: `crates/gui/frontend/src/api.ts`

**Interfaces:**
- Produces: `MonitorReader::current_input(&self, display_index: u32) -> Result<u8>` (Rust), Tauri command `current_input(display_index: u32) -> Result<u8, String>`, frontend `currentInput(displayIndex: number): Promise<number>` — used by `MainScreen` (Task 13).

- [ ] **Step 1: Add the trait method**

Edit `crates/ddc-backend/src/lib.rs`, extending the `MonitorReader` trait:

```rust
/// Read-only monitor/capability discovery, used only by the GUI's
/// configuration wizard. Deliberately separate from `DdcBackend`: the
/// orchestrator's write path (`DdcBackend::set_vcp`) never depends on this
/// trait, so nothing about the already-tested orchestrator changes here.
pub trait MonitorReader {
    fn enumerate(&self) -> Result<Vec<MonitorInfo>>;
    fn input_codes(&self, display_index: u32) -> Result<Vec<u8>>;
    /// Reads VCP feature `0x60` (input select)'s *current* value — lets the
    /// GUI's main screen highlight which input is already active instead of
    /// presenting every input as equally "not yet chosen". Read-only, same
    /// as `enumerate`/`input_codes`: never touches the orchestrator's write
    /// path.
    fn current_input(&self, display_index: u32) -> Result<u8>;
}
```

- [ ] **Step 2: Implement it in `DdcHiMonitorReader`**

Edit `crates/ddc-backend/src/ddchi_reader.rs`, adding the method to the existing `impl MonitorReader for DdcHiMonitorReader` block (after `input_codes`):

```rust
    /// Reuses the same `Ddc` trait `enumerate()` already brings into scope
    /// (see the `use` at the top of this file) — `get_vcp_feature` returns a
    /// `mccs::Value` whose `sl` field is the low byte of the current value,
    /// which is all VCP 0x60's single-byte input codes need (mirrors how
    /// `input_codes` above already treats these codes as plain `u8`s).
    fn current_input(&self, display_index: u32) -> Result<u8> {
        const INPUT_SELECT: u8 = 0x60;
        let mut displays = Display::enumerate();
        let display = displays
            .get_mut(display_index as usize)
            .ok_or_else(|| anyhow!("no display at index {}", display_index))?;
        let value = display
            .handle
            .get_vcp_feature(INPUT_SELECT)
            .map_err(|err| anyhow!("failed to read current input for display {}: {:?}", display_index, err))?;
        Ok(value.sl)
    }
```

- [ ] **Step 3: Verify the workspace still compiles**

Run: `cargo build --workspace`
Expected: fails at this point — `commands.rs` doesn't yet expose the new trait method, but `ddc-backend` itself should compile cleanly. Run `cargo build -p ddc-backend` specifically.
Expected: success (no other type implements `MonitorReader`, so nothing else needs updating).

- [ ] **Step 4: Add the Tauri command**

Edit `crates/gui/src-tauri/src/commands.rs`, adding after `list_inputs`:

```rust
#[tauri::command]
pub fn current_input(display_index: u32) -> Result<u8, String> {
    DdcHiMonitorReader.current_input(display_index).map_err(|err| err.to_string())
}
```

- [ ] **Step 5: Register the command**

Edit `crates/gui/src-tauri/src/main.rs`, in the `invoke_handler` list (around line 320-327):

```rust
        .invoke_handler(tauri::generate_handler![
            commands::list_usb_devices,
            commands::list_monitors,
            commands::list_inputs,
            commands::save_config,
            commands::load_config,
            commands::switch_input,
            commands::current_input,
        ])
```

- [ ] **Step 6: Verify the workspace compiles and tests pass**

Run: `cargo build --workspace && cargo test --workspace`
Expected: build succeeds; all existing tests still pass (no behavior of the existing write path or orchestrator changed).

- [ ] **Step 7: Add the frontend wrapper**

Edit `crates/gui/frontend/src/api.ts`, adding after `switchInput`:

```ts
export const currentInput = (displayIndex: number) => invoke<number>("current_input", { displayIndex });
```

- [ ] **Step 8: Commit**

```bash
git add crates/ddc-backend/src/lib.rs crates/ddc-backend/src/ddchi_reader.rs crates/gui/src-tauri/src/commands.rs crates/gui/src-tauri/src/main.rs crates/gui/frontend/src/api.ts
git commit -m "feat: add current_input command to read the active monitor input"
```

---

### Task 13: `MainScreen.tsx` — restyle, active-input highlight, inline errors

**Files:**
- Modify: `crates/gui/frontend/src/MainScreen.tsx`
- Test: `crates/gui/frontend/src/MainScreen.test.tsx`

**Interfaces:**
- Consumes: `currentInput` (Task 12), `vcpInputLabel` (Task 4), `StatusBadge`/`InlineError` (Task 6).

- [ ] **Step 1: Write the failing test**

Create `crates/gui/frontend/src/MainScreen.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { MainScreen } from "./MainScreen";
import type { Configuration } from "./api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

const config: Configuration = {
  usb_device: "17e9:6000",
  mxkeys_usb_device: "046d:c52b",
  on_usb_connect: "0x11",
  on_usb_disconnect: null,
  on_usb_connect_source_addr: null,
  on_usb_connect_vcp_code: null,
  display_index: 0,
};

describe("MainScreen", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockClear();
  });

  it("highlights the currently active input with a friendly label", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([0x0f, 0x11]) // list_inputs
      .mockResolvedValueOnce(0x11); // current_input

    render(<MainScreen config={config} onReconfigure={() => {}} />);

    const activeButton = await screen.findByText("Active");
    expect(screen.getByText("HDMI 1")).toBeInTheDocument();
    expect(activeButton).toBeDisabled();
  });

  it("shows an inline error when switching fails", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([0x0f, 0x11])
      .mockResolvedValueOnce(0x11)
      .mockRejectedValueOnce("DDC write failed");

    render(<MainScreen config={config} onReconfigure={() => {}} />);
    fireEvent.click(await screen.findByText("Switch"));

    expect(await screen.findByRole("alert")).toHaveTextContent("DDC write failed");
  });

  it("calls onReconfigure when the settings button is clicked", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([0x0f, 0x11]).mockResolvedValueOnce(0x0f);
    const onReconfigure = vi.fn();
    render(<MainScreen config={config} onReconfigure={onReconfigure} />);

    await waitFor(() => screen.getByLabelText("Reconfigure"));
    fireEvent.click(screen.getByLabelText("Reconfigure"));
    expect(onReconfigure).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cd crates/gui/frontend && npm run test -- MainScreen`
Expected: FAIL — no active-input highlight, no friendly labels, no inline error, no `aria-label="Reconfigure"` exist yet.

- [ ] **Step 3: Implement it**

Replace `crates/gui/frontend/src/MainScreen.tsx`:

```tsx
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Keyboard, Monitor as MonitorIcon, Settings } from "lucide-react";
import { Configuration, currentInput, listInputs, switchInput } from "./api";
import { vcpInputLabel } from "./vcpLabels";
import { StatusBadge } from "./components/StatusBadge";
import { InlineError } from "./components/InlineError";

export function MainScreen({ config, onReconfigure }: { config: Configuration; onReconfigure: () => void }) {
  const [inputs, setInputs] = useState<number[]>([]);
  const [active, setActive] = useState<number | null>(null);
  const [mxkeysConnected, setMxkeysConnected] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  const displayIndex = config.display_index ?? 0;

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
    currentInput(displayIndex)
      .then(setActive)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

  useEffect(() => {
    const unlisten = listen<boolean>("mxkeys-status", (event) => setMxkeysConnected(event.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleSwitch = async (value: number) => {
    setError(null);
    try {
      await switchInput(value);
      setActive(value);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex h-full flex-col gap-5 p-5">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-neutral-900 dark:text-neutral-100">KVM Switch</h1>
        <button
          onClick={onReconfigure}
          aria-label="Reconfigure"
          className="rounded-md p-2 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
        >
          <Settings size={18} />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <Keyboard size={16} className="text-neutral-500" />
        <span className="text-sm text-neutral-600 dark:text-neutral-400">MX Keys receiver:</span>
        <StatusBadge status={mxkeysConnected === null ? "unknown" : mxkeysConnected ? "connected" : "disconnected"} />
      </div>

      <div className="flex flex-col gap-2">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-neutral-700 dark:text-neutral-300">
          <MonitorIcon size={16} className="text-emerald-600" />
          Switch input
        </h2>
        <ul className="flex flex-col gap-2">
          {inputs.map((v) => (
            <li
              key={v}
              className={`flex items-center justify-between gap-3 rounded-md border px-3 py-2 ${
                v === active
                  ? "border-emerald-500 bg-emerald-50 dark:border-emerald-500 dark:bg-emerald-950/40"
                  : "border-neutral-200 bg-white dark:border-neutral-700 dark:bg-neutral-900"
              }`}
            >
              <span className="text-sm">{vcpInputLabel(v)}</span>
              <button
                disabled={v === active}
                onClick={() => handleSwitch(v)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {v === active ? "Active" : "Switch"}
              </button>
            </li>
          ))}
        </ul>
      </div>

      <InlineError message={error} />
    </div>
  );
}
```

- [ ] **Step 4: Run it and confirm it passes**

Run: `cd crates/gui/frontend && npm run test -- MainScreen`
Expected: `3 passed`.

- [ ] **Step 5: Run the full frontend suite and build**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all tests pass, build succeeds.

- [ ] **Step 6: Commit**

```bash
git add crates/gui/frontend/src/MainScreen.tsx crates/gui/frontend/src/MainScreen.test.tsx
git commit -m "feat: restyle MainScreen and highlight the active monitor input"
```

---

### Task 14: Update `MANUAL_TEST_GUI.md` with the new scenarios

**Files:**
- Modify: `MANUAL_TEST_GUI.md`

**Interfaces:**
- Produces: an updated manual-test checklist the user runs on real hardware as this plan's final acceptance gate.

- [ ] **Step 1: Add new scenarios to the wizard flow section**

Edit `MANUAL_TEST_GUI.md`, inserting after the existing numbered "Wizard flow" list (after item 6, before the tray item 7):

```markdown
7. **Back navigation:** on the input-mapping step, click the new back arrow
   (top-left). Confirm it returns to the monitor step with the previously
   selected monitor showing a checkmark. Click back again to the MX Keys
   step, then again to the switch-device step; confirm the switch-device and
   MX Keys steps restart their plug-detection flow from scratch (this is
   expected — see this plan's Task 8 back-navigation design).
8. **Inline errors:** temporarily unplug the monitor's DDC connection (or
   otherwise make a DDC call fail) and confirm the wizard shows a red inline
   error message anchored under the relevant step, not a floating toast, and
   that it clears on the next successful action.
9. **Friendly labels:** confirm the switch-device/MX-Keys candidate list
   shows "Logitech (046d:c52b)" or "DisplayLink (17e9:6000)" style labels
   instead of raw hex vendor:product ids, and the input-mapping step's
   dropdowns show "DisplayPort 1"/"HDMI 1" instead of `0xf`/`0x11`.
```

Renumber the former item 7 (tray quick-switch check) to item 10.

- [ ] **Step 2: Add a new scenario to the main screen section**

Edit `MANUAL_TEST_GUI.md`, inserting after the existing "Main screen" item 2 (MX Keys status):

```markdown
3. Confirm the input that matches the monitor's actual current source shows
   an "Active" (disabled) button and a highlighted border, without clicking
   anything — this comes from the new `current_input` DDC read, not from
   memory of the last button clicked. Manually switch the monitor's input
   using the monitor's own physical buttons/remote (bypassing this app
   entirely), then reopen or refresh the main screen; confirm the
   highlighted "Active" input updates to match reality.
```

Renumber the following items accordingly.

- [ ] **Step 3: Commit**

```bash
git add MANUAL_TEST_GUI.md
git commit -m "docs: add manual-test scenarios for back nav, inline errors, and active-input highlight"
```

---

### Task 15: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Full Rust workspace check**

Run: `cargo build --workspace && cargo test --workspace`
Expected: success, all tests green.

- [ ] **Step 2: Full frontend check**

Run: `cd crates/gui/frontend && npm run test && npm run build`
Expected: all Vitest tests pass; `tsc && vite build` succeeds with no type errors.

- [ ] **Step 3: Manual visual pass (no hardware required)**

Run: `cargo tauri dev`
Confirm, purely visually: Inter font is applied, emerald accents appear on primary buttons/active states, the wizard shows a thin emerald progress bar that advances/retreats correctly, step transitions fade in, and dark mode (toggle your OS theme) still looks correct throughout. This does not require real DDC/USB hardware — the wizard can be driven with whatever `list_usb_devices`/`list_monitors` return in this environment (even an empty list is enough to see the layout, loading, and error states).

- [ ] **Step 4: Real-hardware manual test (user must run this)**

Run `cargo tauri build --debug` and walk through the updated `MANUAL_TEST_GUI.md` on the real hardware described in `docs/DECISIONS.md` (LG 34GL750, DisplayLink dongle 17e9:6000, Logitech MX Keys/Unifying receiver). This is the final acceptance gate for this plan and cannot be completed in this environment (no display/USB hardware available here) — **do not consider this plan done until this pass is run and confirmed.**
