---
name: Cloud Drive Bridge
version: 0.1.0
colors:
  bg: "#081120"
  surface: "#0E1B30"
  surface2: "#14263F"
  border: "#22385A"
  text: "#EAF4FF"
  muted: "#93A9C4"
  accent: "#38BDF8"
  accent-dim: "#1D6E96"
  deep: "#0B2A4A"
  warn: "#F5B84C"
  danger: "#F26D6D"
  ok: "#3ED598"
typography:
  font-family: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
  base-size: "15px"
  mono: "ui-monospace, 'Cascadia Mono', Consolas, monospace"
spacing:
  unit: 8px
  radius: 12px
---

# Cloud Drive Bridge — design direction

**One generator direction (taste-skill / hallmark layering):** a deep-navy
harbour at night, crossed by a single sky-cyan bridge arc. The arc is the
product: your bucket, on your desktop, as a drive. No purple/blue AI glow,
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
