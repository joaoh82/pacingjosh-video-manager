# User Guide

A practical walkthrough of Video Manager — a local‑first desktop app for indexing,
searching, tagging, and AI‑editing a large video library.

For the AI‑specific features (transcription, the edit pipeline's planning step,
AI timeline edits, social copy, thumbnail restyle) see the companion
[**AI Features guide**](ai-features.md). For where files are kept on disk, see
[**Data storage**](data-storage.md).

---

## 1. Getting started

### Install

Grab the installer for your OS from the
[Releases page](https://github.com/joaoh82/pacingjosh-video-manager/releases/latest).
FFmpeg is bundled inside the app — there's nothing else to install. Builds are
unsigned, so your OS may show an "unverified developer" prompt the first time
(Windows: **More info → Run anyway**; macOS: **right‑click → Open**).

> Developers running from source instead: start the Rust backend (`cargo run` in
> `backend-rust/`) and the frontend (`npm run dev` in `frontend/`), then open
> `http://localhost:3000`. See the README's *Development* section.

### First‑time setup

On first launch you're taken to a **setup screen**:

1. Click **Browse…** to pick the folder that holds your videos (or type the path).
2. Click **Start Scanning**.
3. The app recursively indexes every supported file and generates thumbnails.
   Progress is shown live with an ETA.

Supported formats: `.mp4`, `.mov`, `.avi`, `.mkv`, `.webm`, `.flv`, `.wmv`.

Your source files are **never moved or copied** — the app only indexes them in
place and stores its own database/thumbnails separately (see
[Data storage](data-storage.md)).

---

## 2. Browsing your library

The **main screen** shows a grid of video cards. Each card has a thumbnail,
duration, resolution, file size, tags, and any production badges.

- **Hover / click** a card to open the **video modal** — a built‑in player with
  seeking, plus the metadata editor and (for shorts) the AI content panel.
- The header has **Rescan**, **Productions**, and **Settings**.

### Rescanning

Click **Rescan** in the header any time you add, remove, or change files in your
video directory. The scan picks up new/changed files and refreshes metadata and
thumbnails, again with live progress.

---

## 3. Searching & filtering

Use the **search bar** to match on filename, location, or notes. The **filter
panel** narrows the grid further:

- **Category** — your own free‑form categories.
- **Tags** — select one or many; results match all selected tags.
- **Production** — show only the clips linked to a given production.
- **Orientation** — portrait / landscape / square.
- **Date range** — filter by created date.
- **Sort** — by date, name, size, or duration (ascending/descending).

---

## 4. Tags, categories & metadata

### Edit a single video

1. Click a video card to open the modal.
2. Click **Edit**.
3. Update **category**, **location**, **tags**, **notes**, or **linked
   productions**.
4. Click **Save**.

### Bulk edit

1. Select several videos with their checkboxes.
2. Click **Bulk Edit** in the bottom toolbar.
3. Set a category; add/remove tags; add/remove production links across all the
   selected videos at once.
4. Click **Apply Changes**.

Bulk edit is the fastest way to drop a batch of raw takes into a production.

---

## 5. Productions

A **production** is a project — a YouTube video, a TikTok, an Instagram post —
that one or more clips belong to. The relationship is many‑to‑many: a clip can
belong to several productions, and a production can collect many clips.

1. Click **Productions** in the header.
2. **Create** a production with a title, platform (YouTube, TikTok, …), and an
   optional published link.
3. Mark it **draft** or **published**.
4. Link clips to it from the video modal or via **Bulk Edit**.

Filtering the grid by a production shows exactly which clips belong to it — handy
when assembling the raw takes for an edit.

---

## 6. Edit & Create Video (the AI pipeline)

This is the headline feature: hand the app your **raw takes + a script**, and it
assembles a finished cut for you. It needs AI keys configured under
**Settings → AI / LLM** (see the [AI Features guide](ai-features.md)).

### Set it up

1. Create a production and add **all the raw takes** of your video to it.
2. Open **Productions** and click the **🎬 clapperboard** on that production.
3. Paste your **script** (Markdown is fine; scene breaks help the editor align
   takes). Optionally add **extra instructions** — e.g. "I warm up by saying 'Hey
   Sarah' — cut that," or "I re‑shot scene 1 at the very end."
4. Configure the run:
   - **Output folder** (required) — where the final video and its edit decision
     list (EDL) JSON are written. Optionally set the **filename**.
   - **Burn in captions** (on by default) — overlays the spoken words, re‑timed
     per clip from the transcript.
   - **Tighten the cut** (optional) — drops long silences and filler ("um"/"uh")
     inside clips, splitting each into speech‑only sub‑clips (jump cuts). Needs
     word‑level timestamps (ElevenLabs/Whisper).
   - **Enhance voice** (optional) — check the noisy takes and set an **intensity**
     slider; the app cleans up only those takes (wind/rumble, hiss, clicks). This
     is FFmpeg DSP, not AI — no extra keys. Each take has a thumbnail and a
     click‑to‑**preview** so you can see/hear it first.
   - **Background music** (optional) — pick a track that loops under the speech
     with **two levels**: a volume for pauses and a lower one for while you talk.
     A "bring music back only after pauses longer than N seconds" control keeps
     short thinking pauses ducked so the music doesn't pop in mid‑sentence.
5. Click **Run pipeline**.

### What happens

The pipeline runs in stages with live progress and an activity log:

1. **Transcribe** every take (with word‑level timestamps where the provider
   supports it).
2. **Plan** the cut — the LLM picks the best takes in script order, trims warm‑up
   intros, drops bad takes.
3. Write the **edit decision list** (per‑clip `video_id` + time ranges) to JSON.
4. **Cut** each clip (burning in captions / enhancing voice as configured).
5. **Stitch** the final video with FFmpeg, mixing in ducked music if provided.

When it finishes you get a per‑clip breakdown, the interactive timeline (below),
and a **Reveal final video** button.

### Output & versions

Each run is written to a numbered version folder inside the folder you chose:

```
<output folder>/productions/v1/<name>.mp4   + <name>.json   (first run)
<output folder>/productions/v2/<name>.mp4   + <name>.json   (next run / re-render)
```

Re‑runs and re‑renders never overwrite each other — they get the next `v<N>`.
Nothing is written to the app's data directory.

---

## 7. Editing the cut on the timeline

Every finished run shows an **editor‑style timeline** (like CapCut). You don't
have to re‑run the whole pipeline to fix the cut — you can adjust it right on the
timeline and **re‑render a new version** from the saved takes (no
re‑transcription, no re‑planning).

### The layout

- An in‑app **preview player** at the top; play it and a **red playhead** tracks
  across the timeline. Click or drag the **ruler** to scrub — the video jumps to
  that point.
- A **Video** track of clip thumbnails, a **Voice** track showing where speech
  is, and a **Music** track whose bar height drops to the ducked level under
  speech.
- **Zoom** in/out and scroll for precision.

### Resize (trim) a clip

1. **Click a clip** to select it (an inspector panel opens).
2. **Drag its left or right edge** to trim — the rest of the timeline holds still
   while you drag, then the cut ripples into place when you release. Great for
   shortening a long‑running take.
3. Or type **exact in/out points** (seconds into the take) in the inspector.
4. **Reset trim** in the inspector reverts to the original range.

### Remove (and restore) a clip

- With a clip selected, click **🗑 Remove from cut**. Removed clips appear as
  chips below the timeline — click a chip to **restore** it.
- You can select several clips (click each) and remove them together.

### Enhance a clip's voice after the fact

Select a clip and toggle **✨ Enhance voice** in the inspector (🎙 marks clips
already enhanced). This applies the same noise‑removal cleanup as the pre‑render
option, only to that clip.

### Music: remove or fade

Click a green **music "burst"** (a stretch where music plays at full level) to
open a small menu:

- **🔇 Remove music here** — ducks the music away in that region.
- **🎚 Fade in / out** — ramps the music smoothly up at the start of the burst
  and back down at the end (no abrupt pops).
- **↺ Keep music as‑is** — clears the action.

Removed regions show red; faded regions show amber. Music edits are **sticky** —
they're remembered on the rendered version and stay applied (shown selected) when
you reopen it or re-render again, including when you also trim/remove clips in the
same pass.

### Ask AI to do it for you

Click **✨ Ask AI** (optionally select clips first to focus the request), then
describe what you want in plain English:

- "Trim the long pause at the end of this clip."
- "Drop the rambling intro."
- "Shorten clip 2 to just the key sentence."
- "Fade the music in at the start and out over the last few seconds."

The AI reads the saved cut and each clip's transcript, proposes the
trims/removals/enhancement and music remove/fade, and **applies them to the
timeline for you to review**. Nothing is rendered until you choose to. See the
[AI Features guide](ai-features.md#ai-timeline-edits) for details.

### Re‑render

When you're happy, click **Re‑render** (the button shows a summary, e.g.
"2 clips, 1 muted, 1 faded"). A **new version** (`v<N+1>`) is cut from the saved
takes. Use **Reset edits** to clear all pending changes.

> While structural edits (trims/removals) are pending, the preview player still
> shows the *last* render — re‑render to see them applied.

---

## 8. YouTube copy

On a finished run, click **Generate copy** to turn the final cut's transcript
into:

- 3 SEO‑optimized **title** options,
- a **description** (hook + keyword summary + hashtags),
- keyword **tags**, and
- **thumbnail‑text** ideas.

Each piece has a one‑click **Copy** button. Click **Regenerate** for a fresh set.
(Powered by your text/LLM provider — see the AI guide.)

---

## 9. Thumbnail builder

Click **Make thumbnail** on a finished run (it reads **Edit thumbnail** once one
has been saved):

1. **Scrub** the slider to grab a real still frame from the final video (the
   preview updates live).
2. Lay **stylized text** on top — font size, outline, text/edge colors, CAPS, and
   **alignment** (left / center / right); thumbnail‑text suggestions are one click
   away.
3. **Position the text anywhere** — drag it directly on the image, or use the
   **X** and **Y** sliders for precision (so you can tuck it into a corner, not
   just move it up and down).
4. Optionally **✨ AI style text** to have the LLM design a punchy treatment
   (colors, gradient, drop shadow, highlight band) — or set a **Band**/**Shadow**
   yourself. **Reset style** returns to plain text.
5. Optionally **✨ AI restyle frame** for a more produced background look (the text
   stays a real overlay). The image provider/model is configurable in Settings.
6. **Download PNG** (1280×720) or **Save to folder** (next to the video).

**Saved thumbnails persist.** Saving writes the finished PNG next to the video,
keeps the (possibly AI‑restyled) background still alongside it, and records the
full builder state — text, position, alignment, colors/style, and the frame
time — with the run. Reopen the run later and the thumbnail comes back exactly as
you left it, ready to re‑edit.

---

## 10. Edit history

Every pipeline run and re‑render is saved per production. Reopen the modal and use
the **History** sidebar to:

- Browse past runs (newest first), with status and clip count.
- Select a run to see its **script**, **edit decision list**, **timeline**, and
  **activity log**, and to **Reveal final video**.
- **Delete** a run — removes its database row *and* its files from disk (the
  video, the EDL JSON, any saved thumbnail/background, and the now‑empty version
  folder).
- Click **＋ New edit** to start another run.

---

## 11. Settings

Open **Settings** from the header.

- **Library** — the video directory and thumbnail options.
- **AI / LLM** — providers, models, and API keys for transcription, text/LLM, and
  thumbnail images, plus the editable copy‑generation and edit‑planning prompts.
  Keys are stored locally and never returned by the API after saving. Full detail
  in the [AI Features guide](ai-features.md).

---

## 12. Tips & troubleshooting

- **No captions / "tighten" / copy available?** Those need **word‑level
  timestamps**. Use **ElevenLabs (Scribe)** or **OpenAI (Whisper)** for
  transcription — Gemini returns plain text only.
- **Pipeline won't start?** A production needs at least one take, a script, and an
  output folder. AI keys must be set under Settings → AI / LLM.
- **Re‑render says "no clips left"?** You removed every clip — restore at least one
  or hit **Reset edits**.
- **Where did my files go?** Final videos live under the output folder you chose
  (`productions/v<N>/`); the app's own data (index, thumbnails, settings) lives in
  a per‑user app‑data folder. See [Data storage](data-storage.md).
- **FFmpeg errors in the activity log?** The bundled FFmpeg is used automatically;
  the log records which binary ran and the tail of any error.
