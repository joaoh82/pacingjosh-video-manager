# FFmpeg Sidecar Binaries

Tauri bundles these as per-platform sidecar binaries alongside the app.
They are **not committed to git** (too large, licensed separately) — run
the fetch script from the repo root to populate this directory:

```bash
# From the repo root:
bash scripts/fetch-ffmpeg.sh       # macOS / Linux
scripts\fetch-ffmpeg.ps1           # Windows PowerShell
```

After running, you should have files with Tauri's required naming convention:

```
ffmpeg-x86_64-pc-windows-msvc.exe
ffmpeg-aarch64-pc-windows-msvc.exe
ffmpeg-x86_64-apple-darwin
ffmpeg-aarch64-apple-darwin
ffmpeg-x86_64-unknown-linux-gnu
ffprobe-x86_64-pc-windows-msvc.exe
…
```

The target-triple suffix is required by `tauri.conf.json` → `bundle.externalBin`.
Tauri strips the suffix at install time, so at runtime the binary is simply
named `ffmpeg` / `ffmpeg.exe` inside the bundled Resource directory.

## Dev mode fallback

If these binaries are missing, the Tauri app will log a warning at startup and
fall back to resolving `ffmpeg`/`ffprobe` via the system `PATH`. This lets you
`cargo tauri dev` without running the fetch script — useful while iterating on
non-scanning code.

## Licensing

- **Windows / Linux**: Use LGPL builds (no `--enable-gpl`) to stay compatible
  with this project's MIT license. Windows builds from
  [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) default to GPL — download
  the "essentials (shared)" LGPL build instead if you plan to redistribute.
- **macOS**: Builds from [evermeet.cx](https://evermeet.cx/ffmpeg/) are GPL
  by default; use the LGPL variant.
- Include FFmpeg's `LICENSE.txt` in your app bundle when redistributing.
