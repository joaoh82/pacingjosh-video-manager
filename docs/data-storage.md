# Where Video Manager stores your data

This page explains exactly what the **desktop app** writes to disk, where it lives
on each OS, and what is *not* stored there. It applies to the packaged Tauri app
(the installers from the [Releases page](https://github.com/joaoh82/pacingjosh-video-manager/releases)).

> Standalone backend dev (`cargo run` from `backend-rust/`) is different: it uses a
> CWD-relative `./data` folder instead of the per-user app-data directory. See
> [Standalone / dev mode](#standalone--dev-mode) below.

## Location

The desktop shell resolves Tauri's per-user **app-data directory** and roots
everything there. The folder is named after the app's bundle identifier,
`com.pacingjosh.video-manager`:

| OS | Location (`<id>` = `com.pacingjosh.video-manager`) |
| --- | --- |
| **Windows** | `%APPDATA%\<id>\` → `C:\Users\<you>\AppData\Roaming\com.pacingjosh.video-manager\` |
| **macOS** | `~/Library/Application Support/<id>/` |
| **Linux** | `$XDG_DATA_HOME/<id>/` → usually `~/.local/share/com.pacingjosh.video-manager/` |

On Windows this is the **Roaming** `AppData` (the app uses Tauri's `app_data_dir()`,
not the Local variant).

## What's in that folder

```
com.pacingjosh.video-manager/
├── config.json        # settings + AI provider keys
├── database.db        # SQLite index (metadata, not the videos themselves)
└── thumbnails/
    └── <checksum>/
        ├── thumb_0.jpg
        └── ...
```

- **`config.json`** — your settings (the indexed video directory, thumbnail
  count/width) **and your AI settings, including the API keys** for Gemini /
  OpenAI / Anthropic / ElevenLabs.
  > ⚠️ **The API keys are stored in plaintext here.** They are write-only over the
  > app's API (never returned once saved), but they are **not encrypted on disk**.
  > Treat this file as sensitive — don't commit it, screen-share it, or include it
  > in a bug report.
- **`database.db`** — a SQLite database holding the **index** of your library: each
  video's file path + extracted metadata (duration, resolution, fps, codec), your
  tags, productions, AI generations, and the edit-pipeline history (edit decision
  lists, logs, generated copy). It stores *references and metadata*, **not** the
  video bytes.
- **`thumbnails/`** — the auto-generated preview frames, organized by each video's
  content checksum.

## What is *not* stored there

- **Your source video files.** They stay wherever your scanned video directory
  points; the app only indexes them in place and never copies or moves them.
- **Rendered edit outputs.** The "Edit & Create Video" pipeline writes the final
  `.mp4`, its `.json` edit decision list, and any saved thumbnail (`…-thumbnail.png`
  plus its background still `…-thumbnail-bg.png`) to the **output folder you
  choose** for that run (`<output>/productions/v1`, `v2`, …). By design,
  **nothing** from the edit pipeline is written to the app-data directory. (The
  thumbnail's editable *state* — text, position, colors — is metadata, saved with
  the run inside `database.db`, so it reopens ready to edit.)
- **FFmpeg.** It ships bundled inside the app (a Tauri sidecar), not in app-data.

## Backup, reset, and uninstall

- **Back up / migrate** your library index and settings by copying the whole
  app-data folder above to the same location on another machine. (Your source
  videos and rendered outputs are separate — back those up from their own folders.)
- **Reset the app to a clean state** by quitting it and deleting the app-data
  folder; it is recreated on next launch. You'll need to re-scan your video
  directory and re-enter your API keys.
- **Uninstalling** the app does **not** remove this folder (per-user data is left
  behind on purpose). Delete it manually if you want it gone.

## Standalone / dev mode

When the backend is run on its own for development (`cargo run` from
`backend-rust/`, as in the [Web Dev Workflow](../README.md#quick-start-web-dev-workflow)),
it does **not** use the per-user app-data directory. Instead it roots
`database.db`, `thumbnails/`, and `config.json` under a CWD-relative `./data`
folder (i.e. `backend-rust/data/`), and `backend-rust/.env` can override the
database/thumbnail paths and the video directory.

## For maintainers

The desktop app-data directory is resolved in
[`src-tauri/src/main.rs`](../src-tauri/src/main.rs) via Tauri's
`app.path().app_data_dir()` and passed to the backend's `run_blocking`. The
backend roots `config.json`, `database.db`, and `thumbnails/` at that directory in
[`backend-rust/src/config.rs`](../backend-rust/src/config.rs)
(`ConfigManager::new` / `Settings::from_env_with_base`). The bundle identifier
that names the folder is set in
[`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json).
