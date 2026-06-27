# Launch & promotion plan — Video Manager

A go-to-market plan and ready-to-paste copy for promoting the open-source
[Video Manager](https://github.com/joaoh82/pacingjosh-video-manager) project across
LinkedIn, Hacker News, Reddit, and Twitter/X.

**Positioning chosen:** builder's journey spine · lean into the honest solo/AI-assisted
side-project framing · weave in the Pacing Josh running channel.

---

## Core narrative

> "I just wanted to stop losing my own running videos. Four months and one Rust rewrite
> later, it's an open-source AI video editor that cuts a finished video straight from
> your script."

Three things make this share-worthy — every post should hit at least one:

1. **The mutation** — a folder-indexer that accidentally became a video editor. People
   love watching scope creep with a happy ending.
2. **The honesty** — solo, evenings, built largely with Claude Code, "a side project that
   got out of hand." Pre-empts criticism *and* is currently the most-engaged-with kind of
   dev story.
3. **The craft** — Rust backend embedded in Tauri, LLM-driven edit decision list, FFmpeg
   orchestration, transcript-timestamp music ducking. Real engineering meat.

### The phases (the journey, in order)

- **Feb 2026 — Phase 1: Index & search.** Started as Python/FastAPI + Next.js. Scan a
  folder, pull metadata with FFmpeg, generate thumbnails, tag and full-text search.
- **Feb–Apr 2026 — Phase 2: Make it real software.** Rewrote the backend in Rust
  (Actix-web), then wrapped the whole thing in Tauri 2.0 → one native installer for
  Windows/macOS/Linux, FFmpeg bundled.
- **May 2026 — Phase 3: AI social copy.** One-click titles/descriptions/hashtags from a
  clip's transcript; orientation-aware filtering.
- **June 2026 — Phase 4: The AI editing pipeline.** Paste a script → transcribe takes →
  LLM assembles the best cut → FFmpeg stitches it. Plus a CapCut-style timeline, voice
  cleanup, ducked music, AI thumbnail builder, and tag-driven CI/CD releases.
- **Now: v1.2.0**, ~4.5 months, 72 commits, signed-ish installers for Win/macOS/Linux.

---

## Sequencing (stagger over ~1 week — don't blast everywhere at once)

| Day | Platform | Goal | Why this order |
|---|---|---|---|
| **Day 1 (Tue–Thu AM)** | **LinkedIn** | Flagship story, warm audience | Your network is the safest, highest-signal launch. Get the narrative + first reactions here. |
| **Day 2–3** | **Twitter/X** | Reach builders, repurpose the thread | Lower stakes; the thread reuses the LinkedIn beats. Tag #rustlang #buildinpublic. |
| **Day 3–4** | **r/rust + r/SideProject** | Technical + indie-maker validation | Two angles, two days. Don't double-post the same text. |
| **Day 5 (weekday ~8–10am ET)** | **Hacker News (Show HN)** | The big swing | Save HN for when repo, README, and demo GIF are airtight — HN traffic is unforgiving and one-shot. |
| **Rolling** | r/selfhosted, r/opensource, creator subs | Long-tail | Post only where it genuinely fits; engage in comments. |

## Pre-launch checklist

- [x] **Demo GIF/MP4** — `images/demo.gif` exists. Single most important asset; lead with it everywhere media is allowed.
- [x] **README hero** — strong already.
- [ ] **Pin a GitHub issue** labeled "Roadmap / feedback wanted" as a landing spot for commenters.
- [ ] **Repo topics** on GitHub: `rust`, `tauri`, `video-editing`, `ffmpeg`, `ai`, `nextjs`, `local-first`, `desktop-app`.
- [ ] Be **available ~3 hours** after each post to reply. First-hour engagement drives everything.

## Platform norms

- **LinkedIn** down-ranks posts with external links in the body — **put the repo link in the first comment** and say "link in comments."
- **Hacker News** — no hype words, no emoji, plain title. Honesty rewarded; "marketese" punished. The real pitch goes in *your own first comment*.
- **Reddit** — each sub has self-promo rules; never paste identical text across subs (spam flags). Engage as a person.

---

## 1) LinkedIn — flagship post

> Four months ago I just wanted to stop losing my own running videos.
>
> Today it's an open-source AI video editor. I did not plan this. 👇
>
> I run, I film it, and I post to a small YouTube channel (Pacing Josh). The problem was never the running — it was the footage. My drive was a swamp of raw takes: Take 1, Take 2, the one where I said "hey guys" three times before getting it right. Finding a usable clip took longer than the actual run.
>
> So I built a tiny local app to index and search my own library. That was supposed to be the whole thing.
>
> It was not the whole thing. The phases, roughly:
>
> ▸ Phase 1 — Index & search. Point it at a folder, it pulls metadata with FFmpeg, makes thumbnails, lets me tag and full-text search. A librarian for video.
>
> ▸ Phase 2 — Make it real software. I rewrote the backend in Rust and wrapped the whole thing in Tauri 2.0 — so it's now one native installer for Windows, macOS and Linux, with FFmpeg bundled inside. Nothing to configure.
>
> ▸ Phase 3 — Let AI do the boring copy. One click turns a clip's transcript into titles, descriptions and hashtags.
>
> ▸ Phase 4 — the part I genuinely didn't see coming. A full editing pipeline. I paste my script, point it at the raw takes, and it transcribes everything, asks an LLM to assemble the best cut (newest clean takes, warm-up "hey guys" intros trimmed), and stitches the final video with FFmpeg. Then there's a CapCut-style timeline to fine-tune, automatic voice cleanup, background music that ducks under my voice, and an AI thumbnail builder.
>
> Here's the honest part: I build this in the evenings, mostly pair-programming with Claude Code, between training runs. It's not a startup. It's a side project that got out of hand — and that's exactly why I open-sourced it (MIT). Maybe it saves someone else from footage swamp.
>
> Where it is now: v1.2.0, real installers on the releases page, and it edits the videos for my own channel today.
>
> What's next: an analytics dashboard, export/import, maybe a mobile companion.
>
> If you wrangle a pile of video — or you just enjoy watching a weekend project mutate into something bigger than intended — the repo's in the comments. Stars and brutal feedback equally welcome. ⭐
>
> Stack: Rust · Actix-web · Tauri 2 · Next.js · SQLite · FFmpeg
>
> #opensource #rustlang #buildinpublic #ffmpeg #sideproject

**First comment (post immediately after):**

> Repo (MIT, installers for Win/macOS/Linux): https://github.com/joaoh82/pacingjosh-video-manager
>
> And the channel that started it all: https://www.youtube.com/@pacingjosh

---

## 2) Hacker News — Show HN

**Title** (plain, no hype):

```
Show HN: Local-first desktop app that edits video from your script (Rust/Tauri)
```

**Your first comment (post right after submitting):**

```
Hi HN. I run and film it for a small YouTube channel, and I was drowning in raw
takes — multiple attempts per line, warm-up "hey guys" intros, no good way to
find or assemble anything. I built this to dig myself out, and it grew further
than I expected.

What it does:
- Indexes a local video folder (FFmpeg metadata + thumbnails), with tagging and
  full-text search.
- The part I'm actually curious to get feedback on: an "edit from script"
  pipeline. You paste a script and point it at your raw takes; it transcribes
  each take with word-level timestamps, asks an LLM to assemble an edit decision
  list (pick the newest clean take per line, trim warm-ups), then cuts and
  concatenates with FFmpeg. After that there's a CapCut-style timeline to trim/
  remove clips by hand or with a natural-language prompt, optional voice cleanup,
  and background music that ducks under speech using the transcript timestamps
  rather than audio detection (so quiet recordings still duck correctly).

How it's built:
- Rust (Actix-web) backend compiled as a library and embedded in a Tauri 2.0
  shell on a dedicated thread; Next.js static-exported into the WebView. Diesel/
  SQLite. FFmpeg bundled as a sidecar so end users install nothing.

Honest caveats:
- It's a solo side project I work on in the evenings, built largely with Claude
  Code. The installers are unsigned, so Windows SmartScreen / macOS Gatekeeper
  will warn you (instructions in the README).
- Transcription/LLM steps use your own API keys (ElevenLabs/OpenAI/Gemini for
  transcription, Gemini/OpenAI/Anthropic for the cut). Everything else is local;
  your videos never leave your machine.

Repo (MIT): https://github.com/joaoh82/pacingjosh-video-manager

Happy to go into the FFmpeg filter graphs, the EDL format, or the Tauri-embeds-
Actix setup — ask away.
```

---

## 3) Reddit

### r/rust — technical angle

```
Title: I embedded an Actix-web backend inside a Tauri 2.0 app to build a
local AI video editor — some notes on the architecture

Body:
I've been building a local-first video manager/editor to handle the footage for
my YouTube channel, and the Rust side turned out more interesting than I
expected, so I figured r/rust might enjoy the writeup.

The shape:
- The Actix-web backend is compiled as a *library* (run_blocking(BackendPaths))
  and the Tauri shell boots it on a dedicated OS thread, picks a free localhost
  port, and injects it into the WebView. So the same crate is both a standalone
  binary and an embedded server.
- Diesel + r2d2 pool over SQLite, shared via Actix app data.
- FFmpeg is shelled out for everything heavy, but the interesting logic is pure
  and testable: building the music-ducking volume automation expression from
  transcript word timestamps, clamping LLM-proposed edit ranges to real take
  durations, parsing/validating the edit decision list, etc.
- Bundled FFmpeg as a Tauri sidecar so users install nothing.

The AI editing flow: transcribe takes → LLM returns an edit decision list →
clamp/validate → cut + concat with FFmpeg. The LLM never touches bytes, just
proposes ranges, which keeps it deterministic and cheap to re-render.

It's MIT, solo, built mostly in the evenings (and with a lot of Claude Code).
Clippy + tests are the CI gates. Repo:
https://github.com/joaoh82/pacingjosh-video-manager

Happy to take architecture critique — especially on the embed-server-in-Tauri
pattern, which I'm not 100% sure is the "right" way to do it.
```

### r/SideProject — journey angle

```
Title: My "organize my videos" weekend project accidentally became an AI video editor (4 months, open source)

Body:
I film myself running for a small YouTube channel and was buried in raw takes.
So I built a little local app to index and search them. That was meant to be it.

Then it kept growing:
1. Index + search a video folder (thumbnails, tags, metadata)
2. Rewrote the backend in Rust + wrapped it in Tauri → real cross-platform
   desktop installers
3. One-click AI social copy from a clip's transcript
4. A full "paste your script → get a finished cut" pipeline: it transcribes the
   takes, an LLM picks the best ones, FFmpeg stitches it, and there's a timeline
   to fine-tune by hand or with a prompt.

Now at v1.2.0 with installers for Win/macOS/Linux. It's a solo evenings-and-
weekends thing built largely with Claude Code, fully open source (MIT). Not
trying to make it a business — just sharing in case it's useful or interesting.

Repo + demo GIF: https://github.com/joaoh82/pacingjosh-video-manager

Would love feedback on what to build next — current ideas are an analytics
dashboard and export/import.
```

### Other subs (adjust angle, post sparingly, read each sub's rules first)

- **r/selfhosted / r/opensource / r/DataHoarder** → lead with *local-first, your videos
  never leave your machine, MIT, bundled FFmpeg*. Retitle: "Local-first desktop app to
  index, search and AI-edit a large video library (Rust/Tauri, MIT)."
- **r/NewTubers / r/youtubers** → strict on self-promo; only post if framed as a *free
  tool you made for your own channel and are giving away*, lead with the creator benefit
  ("paste your script, it assembles the cut"), skip the tech. Check each sub's promo-day
  rules first.

---

## 4) Twitter / X

**Hook tweet (standalone, with the demo GIF):**

```
4 months ago I built a tiny app to organize my running videos.

It is now an open-source AI video editor that cuts a finished video from your script.

I did not plan this. 🧵
```

**Thread:**

```
2/ The problem: I film myself running, post to a small YouTube channel, and my
drive was a swamp of raw takes. Finding one usable clip took longer than the run.

3/ So: a local app to index + search my footage. Thumbnails, tags, search.
Supposed to be the whole project. It was not.

4/ Then I rewrote the backend in Rust and wrapped it in Tauri 2.0 → one native
installer for Win/macOS/Linux, FFmpeg bundled. Suddenly it was real software.

5/ Then the part I didn't see coming: paste a script, point it at your raw
takes, and it transcribes everything, an LLM picks the best cut, and FFmpeg
stitches the final video. Warm-up "hey guys" intros trimmed automatically.

6/ Plus a CapCut-style timeline, voice cleanup, music that auto-ducks under your
voice, and an AI thumbnail builder.

7/ Honest bit: solo side project, built in the evenings between runs, mostly
pair-programming with @claudeai's Claude Code. Open-sourced (MIT) because it
might save someone else from footage swamp.

8/ v1.2.0, installers ready, MIT. Stars and brutal feedback equally welcome 👇
https://github.com/joaoh82/pacingjosh-video-manager

Built with: Rust · Tauri 2 · Next.js · FFmpeg
#rustlang #buildinpublic
```

---

## 5) Comment reply bank (for the inevitable questions)

- **"Why not just use Premiere / DaVinci / CapCut?"** → "Those are great editors but they
  don't *manage* a library or assemble a first cut from a script — this does the boring
  80% (find the takes, pick the clean ones, rough-cut), then you fine-tune. It's a
  pre-editor, not a replacement."
- **"Is my data private?"** → "Videos never leave your machine — everything local. Only
  transcription + the edit-planning step call an API, with your own keys, and only on
  text/audio you opt into."
- **"Why are the installers unsigned?"** → "Solo side project, no signing certs yet.
  README has the SmartScreen/Gatekeeper steps. PRs to wire up signing welcome."
- **"Did AI write all of it?"** → "Heavily AI-assisted (Claude Code), but I architected
  and reviewed it — the testable core logic, the Tauri-embeds-Rust design, the FFmpeg
  filter graphs are deliberate. Being upfront about that is kind of the point."
