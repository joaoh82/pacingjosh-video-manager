# App Icons

Generate platform icons from the repo's logo:

```bash
# From the repo root, with tauri-cli installed:
cargo tauri icon images/Logo.png
```

This writes `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`,
`icon.ico`, and the platform-specific `.png` variants into this directory.
They are referenced from `tauri.conf.json` → `bundle.icon`.

Icons are git-ignored; regenerate them after updating the logo.
