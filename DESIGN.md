---
name: Cloud Drive Bridge
version: 0.5.0
colors:
  # V1 Essential Light (owner pick 2026-11-02, from the approved Stitch
  # set, project 12348950776923825852): warm off-white, NOT plain white.
  bg: "#FAF9F6"
  surface: "#FFFFFF"
  surface2: "#F8FAFC"
  border: "#E2E8F0"
  text: "#0F172A"
  muted: "#64748B"
  accent: "#0284C7"
  accent-dim: "#0369A1"
  accent-light: "#E0F2FE"
  navy: "#081120"
  sky: "#38BDF8"
  teal: "#3ED598"
  warn: "#F59E0B"
  danger: "#EF4444"
  ok: "#10B981"
typography:
  font-family: "'Inter', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
  base-size: "15px"
  mono: "ui-monospace, 'Cascadia Mono', Consolas, monospace"
spacing:
  unit: 8px
  radius: 14px
---

# Cloud Drive Bridge — design direction

**GUI theme (v0.5.0): V1 Essential Light** — the owner's suite-wide pick
from the approved Stitch designs. Warm off-white surfaces (#FAF9F6),
white cards with soft depth, deep ink text, sky accent, teal success;
deep navy survives in the logo, the sidebar brand and the primary
action gradient. The earlier deep-navy dark theme (v0.1–v0.2) is
retired. No purple/blue AI glow,
no glass, no stock photos. Colour means things:

- **Sky cyan `#38BDF8`** = the bridge — connect, mount, the primary action.
- **Deep navy `#0B2A4A`→`#081120`** = the cloud side: storage, night, depth.
- **Mint `#3ED598`** = verified / healthy / mounted.
- **Amber `#F5B84C`** = guidance and honest caveats (missing driver, expiry).
- **Red `#F26D6D`** = destructive actions and failures only.

**Identity:** deep navy + sky cyan, deliberately distinct from Network
Diagnostics (emerald/gold) and CleanReach (ink/mint/amber). Logo: a cyan
bridge arc with two suspension lines over a drive deck, drawn — not
emoji, not a stock cloud.

**Screens (one job each):** Profiles (add bucket → test → keys to the OS
keychain) → Browser (navigate, search, transfer, share, pin) → Mount (one
big **MOUNT AS DRIVE** action, cache, watchdog + auto-mount toggles,
driver guidance) .

**Rules:**
- The hero button says the outcome: **🌉 MOUNT AS DRIVE**. Never "Submit".
- Every job reachable in ≤3 clicks: profile → test → mount / browse.
- Honesty is UI copy: "Android = browse + transfer, no mount (a true
  kernel mount needs root)", "share links expire", "multipart ETags are
  not plain MD5" appear where the user acts, not in fine print.
- Keys are explained where entered: "stored in your OS keychain — never
  in a file, never logged."
- Emoji sparingly as wayfinding in headings/buttons (owner preference),
  never as data.
- Dark theme only in v0.1. System fonts. Listing works offline from
  cache guidance; transfers show progress and can be cancelled.
