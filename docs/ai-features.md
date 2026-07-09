# AI Features

Video Manager uses AI in several places — transcription, assembling a cut from
your script, editing that cut with natural language, generating social/YouTube
copy, and restyling thumbnails. This guide explains what each feature does, which
provider/key it needs, and how to get good results.

For the general walkthrough see the [**User Guide**](user-guide.md). For where
keys and data are stored, see [**Data storage**](data-storage.md).

> AI features are part of the **desktop app**. They call third‑party APIs using
> the keys you provide; nothing is sent anywhere until you trigger an action.

---

## 1. Providers, models & keys

Everything is configured under **Settings → AI / LLM**. There are four
independent provider slots, each with a model field, plus the API keys.

| Slot | What it powers | Providers | Example models |
| --- | --- | --- | --- |
| **Transcription** | Speech‑to‑text for the edit pipeline and per‑video copy | ElevenLabs, OpenAI, Google Gemini | `scribe_v1`, `whisper-1`, `gemini-2.0-flash` |
| **Text / LLM** | Planning the cut, AI timeline edits, social & YouTube copy, ✨ AI thumbnail text styling | Google Gemini, OpenAI, Anthropic | `gemini-2.0-flash`, `gpt-4o`, `claude-sonnet-4-6` |
| **Image** | ✨ AI thumbnail frame restyle | Google Gemini, OpenAI (GPT Image) | `gemini-2.5-flash-image`, `gpt-image-2` |
| **Embedding** | ✨ Semantic search (ranking videos & productions by meaning) | OpenAI, Google Gemini | `text-embedding-3-small`, `text-embedding-004` |

### API keys

Paste keys for the providers you intend to use:

- **ElevenLabs** — transcription (Scribe).
- **Gemini** — transcription, text/LLM, and/or image.
- **OpenAI** — transcription (Whisper), text/LLM, and/or image (GPT Image).
- **Anthropic** — text/LLM (Claude).

Keys are stored **locally** in `config.json` and are **never returned** by the
API after saving (the settings page only shows whether each key is set). See
[Data storage](data-storage.md) for the exact location.

### ⚠️ Word‑level timestamps matter

Several features depend on **word‑level timestamps** in the transcript:

- Burned‑in **captions**
- **Tighten the cut** (removing silences/filler)
- **YouTube copy** and **AI timeline edits** (which read each clip's words)

**ElevenLabs (Scribe)** and **OpenAI (Whisper)** return word timestamps.
**Gemini** returns plain text only — pick ElevenLabs or Whisper for transcription
if you want the timestamp‑driven features.

---

## 2. Per‑video social copy

In the **video modal** (click any video), the AI content panel transcribes the
clip and generates short‑form social copy from the transcript — **thumbnail‑text
ideas** plus **Instagram / TikTok / YouTube Short** titles, descriptions, tags,
and hashtags. It's geared toward portrait/shorts clips.

- Uses your **transcription** + **text/LLM** providers.
- The result is saved with the video; regenerate any time.

---

## 3. The edit pipeline (planning the cut)

When you run **Edit & Create Video** (see the [User Guide](user-guide.md#6-edit--create-video-the-ai-pipeline)),
the LLM does the creative assembly:

1. Every take is transcribed (with timestamps where supported).
2. The transcripts + your **script** + any **extra instructions** are sent to the
   text/LLM provider.
3. The model returns an **edit decision list** — which take to use for each part
   of the script, in order, with in/out time ranges and a short reason per clip.
4. The app validates that list against the real takes (clamping ranges, dropping
   anything invalid) before cutting.

### The editable edit prompt

The planning prompt is **fully editable** in Settings → AI / LLM. It supports the
tokens `{script}`, `{transcripts}`, and `{instructions}` (each is appended
automatically if you remove it). The model must return JSON with a `scenes` array
of clips. A **Reset to default** button restores the built‑in prompt.

Tips for good cuts:

- Put **scene breaks** in your script so the planner can align takes.
- Use **extra instructions** for things the script doesn't say — warm‑up phrases
  to cut, re‑shoot ordering, tone.
- Provide clean takes; the planner prefers the newest clean take of each line.

---

## 4. AI timeline edits

After a render, you can adjust the cut by **asking in plain English** instead of
dragging clips by hand. On the interactive timeline, click **✨ Ask AI**.

### How it works

1. (Optional) **Select one or more clips** first to focus the request — the panel
   shows "Focusing on clips 2, 3."
2. Type an instruction and hit **Apply**.
3. The backend builds a prompt from the **saved cut** — each clip's take, source
   range, duration, voice‑enhanced flag, and **spoken text** — plus the music
   regions, and sends it with your instruction to the text/LLM provider.
4. The model returns a structured plan: clips to **trim** (new in/out),
   **remove**, or **enhance**, and music regions to **remove** or **fade**.
5. The plan is **validated** against the real cut (unknown clips dropped, ranges
   clamped to each clip's range) and **applied to the timeline for review**.
6. Nothing renders until you click **Re‑render**.

It reuses the saved transcripts, so this costs **one LLM call** — no
re‑transcription.

### Example prompts

- "Trim the long pause at the end of clip 3."
- "Drop the rambling intro and the clip where I stumble."
- "Shorten clip 2 to just the sentence about pacing."
- "Cut everything after 'thanks for watching' in the last clip."
- "Fade the music in at the start and out over the last few seconds."
- "Remove the music during the intro."

### Notes & limits

- The AI can only **trim within a clip's existing range, remove it, enhance it, or
  adjust music** — it doesn't re‑plan the whole edit or pull in new footage. For a
  full re‑plan, run a new pipeline.
- It works best when the transcript has **word timestamps** (so it can reason
  about specific lines). With plain‑text transcripts it can still remove/enhance
  whole clips and adjust music.
- You can always tweak the applied plan by hand before re‑rendering, or
  **Reset edits**.

---

## 5. YouTube copy (long‑form)

On a finished run, **Generate copy** turns the **final cut's transcript** into
long‑form YouTube copy: 3 SEO title options, a description (hook + keyword
summary + hashtags), keyword tags, and thumbnail‑text ideas. Uses your text/LLM
provider; saved with the run; **Regenerate** for a new set.

This needs a transcript with timestamps to reconstruct the final cut's words —
i.e. an ElevenLabs/Whisper transcription, not Gemini.

---

## 6. AI thumbnail styling

The **thumbnail builder** has two independent AI helpers. Both keep your text a
real canvas overlay, so the words always stay crisp and editable.

### ✨ AI restyle frame (image model)

After grabbing a frame, click **✨ AI restyle frame** to send that still to your
**image** provider/model for a more produced, cinematic background.

- **OpenAI GPT Image** renders/edits photos more permissively and handles text in
  images best; **Gemini** is cheaper.
- Editing close‑up shots of **real, identifiable faces** may be refused by either
  provider — that's a provider policy, not an app limitation.
- Requires the matching API key (Gemini or OpenAI).

### ✨ AI style text (text model)

Click **✨ AI style text** to have your **Text / LLM** provider design an
eye‑catching treatment for the caption — bold colors, an optional top‑to‑bottom
gradient, a soft drop shadow, and an optional colored highlight band — instead of
plain white‑with‑an‑outline. You can add a one‑line direction ("bold red and
aggressive", "clean minimal white") and the model tailors the look to the
caption and the video's topic. The result drops into the editor's controls, so
you can tweak colors, alignment, position, and the band/shadow afterwards, or
**Reset style** to go back to plain text. Uses your text LLM key only — no image
generation, no extra cost beyond a small completion.

---

## 7. Semantic search (natural‑language search)

The videos page has a **✨ Semantic** toggle next to the search bar (and the
Production Manager has one on its search box). With it on, you can search by
**meaning** instead of exact keywords:

- *"me running in the snow"* → matches videos whose location/notes/tags or
  transcript describe a snowy run, even if none of those exact words appear.
- *"video where I talk about parenting"* → matches by what was **said** (the
  transcript), not the filename.

### How it works

1. Each video is reduced to a text **document** — filename, category, location,
   notes, tags, its productions, and its transcript (from the AI content panel or,
   for takes, the edit pipeline). Each production's document is its title,
   platform, latest script, generated copy, and its takes' transcripts.
2. Those documents are **embedded** into vectors by your chosen embedding provider
   and cached in the database. Your query is embedded the same way, and results
   are ranked by **cosine similarity** — all locally, no per‑search LLM call.

### Building the index

Before the first search, build the index under **Settings → AI / LLM →
Semantic search index → Rebuild index**. It shows how many videos/productions are
indexed for the current model. Rebuilds are **incremental**: only documents whose
text changed (or that were embedded with a different model) are re‑embedded, so
running it again after a scan or after generating transcripts is cheap. Switching
the embedding provider/model requires a rebuild (vectors from different models
aren't comparable).

**Transcribe missing (optional).** Because ranking rides on the transcript, a
library of un‑transcribed talking‑head clips won't discriminate well. Tick
**"Transcribe videos with no transcript first"** before Rebuild to run a
transcription pre‑pass over every clip that has no transcript yet (using your
configured transcription provider). It's much slower and costs transcription‑API
money per clip, but it's what makes "the one where I talk about X" findable.
Silent / action clips (no speech) are skipped automatically, and
already‑transcribed videos are never redone, so re‑running it is cheap.

**Describe visuals (optional).** To make **visual** content searchable — "running
in the snow", "on the beach", "indoor gym" — tick **"Describe visuals for videos
with no description first"**. Before indexing, it sends a few of each clip's
**thumbnails** to your text/LLM provider (Gemini/OpenAI/Anthropic — all
multimodal) and asks it to caption the scene, subjects, and activity plus a set
of visual tags; that caption is stored and folded into the searchable document.
This is what lets a query with **no matching transcript** still find the right
footage. It's slower and costs LLM‑API money per clip, reuses the thumbnails you
already generated (no re‑encoding), and already‑described videos are never redone.
You can tick both pre‑passes at once.

### Notes & limits

- **Only videos with describable text are indexed** — a transcript, tags, notes, a
  category, a *descriptive* filename, or a visual description. Raw clips whose only
  text is a camera filename (e.g. `GX011916.MP4`, `IMG_1234.MOV`) are **skipped on
  purpose**: those names are meaningless and would otherwise embed into one dense
  cluster that dominates every query (making every search return the same clips).
  So the *indexed* count is usually lower than the total until you add tags/notes,
  transcribe, or **describe visuals**.
- **Visuals need the "Describe visuals" pass.** Out of the box, matching is over
  **text only**, so a purely visual query ("running in the snow") only lands if
  some text describes it. Run **Describe visuals** (above) to caption your footage
  and close that gap; otherwise the "what did I talk about" (transcript) case works
  far better than "what do I see".
- **Filters don't apply** to semantic results — it ranks the indexed videos by the
  query and returns the top matches (no pagination).
- Needs an **OpenAI or Gemini** key for embeddings; **Describe visuals** also needs
  a key for your text/LLM provider, and **Transcribe missing** one for your
  transcription provider.

---

## 8. What is *not* AI

A couple of features look AI‑ish but run entirely through **bundled FFmpeg
filters** — no keys, no network, no cost:

- **Enhance voice** — noise removal (high‑pass for wind/rumble, broadband
  denoise, de‑click, a clarity shelf), with a single intensity slider. Available
  per take before rendering and per clip on the timeline.
- **Tighten the cut** — dropping long silences/filler. (It *uses* the transcript's
  timestamps, but the cutting itself is deterministic FFmpeg work.)
- **Music ducking / fades** — driven by the known speech timestamps, applied as a
  deterministic volume automation.

---

## 9. Cost & privacy

- **Local‑first.** Your video index, thumbnails, and settings (including keys)
  stay on your machine. Source videos and rendered outputs never leave it.
- **What gets sent to providers:** extracted **audio** (for transcription) and
  **text** (transcripts, your script/instructions/prompts) for planning, copy, and
  timeline edits, and thumbnail **text styling**; a single **still frame** for
  thumbnail frame restyle. Only when you trigger the action.
- **Cost scales** with the number/length of takes (transcription + planning) and
  the size of prompts (copy, timeline edits). Re‑renders and manual timeline edits
  reuse saved transcripts, so they don't re‑transcribe — the only paid AI call on
  a re‑render is **Ask AI** (one LLM call), and only if you use it.

---

## 10. Troubleshooting

| Symptom | Likely cause / fix |
| --- | --- |
| Captions / Tighten / Copy unavailable or empty | Transcript has no word timestamps — use **ElevenLabs (Scribe)** or **OpenAI (Whisper)**. |
| "Ask AI" returns no changes | The instruction didn't map to an edit, or referred to clips that don't exist. Be specific; select the target clips first. |
| Pipeline / copy fails with an auth error | The relevant **API key isn't set** (or is invalid) in Settings → AI / LLM. |
| Thumbnail restyle refused | The image provider declined (often close‑up real faces). Try the other provider or a wider frame. |
| Planner picks odd takes | Add **scene breaks** to the script and use **extra instructions**; consider editing the **edit prompt** in Settings. |
