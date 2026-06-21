# Video Log Manager — demo video (Remotion)

This [Remotion](https://www.remotion.dev/) project renders the animated product
demo shown at the top of the repo's main `README.md`.

It composes the real app screenshots (from `../images/`, copied into `public/`)
into a narrated, motion-graphics walkthrough: intro → library → AI social copy →
productions → the AI edit pipeline (motion-graphics explainer) → the Edit & Create
Video result screen → AI/LLM settings → outro.

## Output

- `../images/demo.mp4` — full-quality 1920×1080 @ 30fps walkthrough (~26s)
- `../images/demo.gif` — optimized 720px @ 12fps GIF used as the README hero

## Develop

```bash
npm install
npm run dev        # opens Remotion Studio to preview/scrub the "Demo" composition
```

## Re-render

```bash
# 1. MP4 (writes ../images/demo.mp4)
npm run render

# 2. GIF — derived from the MP4 with an optimized palette (needs ffmpeg on PATH)
cd ../images
ffmpeg -i demo.mp4 -vf "fps=12,scale=720:-1:flags=lanczos,palettegen=max_colors=128:stats_mode=diff" -y _pal.png
ffmpeg -i demo.mp4 -i _pal.png -lavfi "fps=12,scale=720:-1:flags=lanczos,paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" -y demo.gif
rm _pal.png
```

## Structure

- `src/Demo.tsx` — the timeline: sequences each scene with cross-fades
- `src/scenes/` — `IntroScene`, `FeatureScene` (reused for the screenshot scenes),
  `PipelineScene`, `OutroScene`
- `src/components/` — `Background`, `Window` (the macOS-style screenshot frame), `Icons`
- `src/theme.ts` — palette, dimensions, fps
- `public/` — app screenshots + logo loaded via `staticFile()`
