# GUI — EXPERIMENTAL, NOT IN THIS RELEASE, NOT VERIFIED

This folder holds the Tauri 2 desktop GUI experiment for Cloud Drive
Bridge. Per the owner's CLI-first directive (2026-11-02, "fix it, no
gui"):

- It is **not built** by the release workflow and **not distributed**.
- It has **never been verified end-to-end** — treat every screen as a
  design sketch, not a working product.
- The shipped product is the **`cdb` CLI** (see the repository README),
  proven end-to-end against a real S3 server by `scripts/e2e-minio.sh`.

The GUI may return later on top of the same cdb-core engine. Until a
release page actually lists GUI installers, there is no GUI.
