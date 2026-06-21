import React from "react";
import {
  AbsoluteFill,
  interpolate,
  Sequence,
  useCurrentFrame,
} from "remotion";
import { theme } from "./theme";
import { Background } from "./components/Background";
import { IntroScene } from "./scenes/IntroScene";
import { FeatureScene } from "./scenes/FeatureScene";
import { PipelineScene } from "./scenes/PipelineScene";
import { OutroScene } from "./scenes/OutroScene";
import {
  FilmIcon,
  KeyIcon,
  LayersIcon,
  SearchIcon,
  SparklesIcon,
} from "./components/Icons";

const FADE = 14;

/** Fades a scene's content in at the start and out at the end so adjacent,
 *  slightly-overlapping sequences cross-fade over the shared background. */
const SceneFade: React.FC<{
  durationInFrames: number;
  children: React.ReactNode;
}> = ({ durationInFrames, children }) => {
  const frame = useCurrentFrame();
  const opacity = interpolate(
    frame,
    [0, FADE, durationInFrames - FADE, durationInFrames],
    [0, 1, 1, 0],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );
  return <AbsoluteFill style={{ opacity }}>{children}</AbsoluteFill>;
};

// Every scene as a render fn + its duration. Each scene starts FADE frames
// before the previous one ends, producing a cross-fade.
const SCENES: { dur: number; render: (dur: number) => React.ReactNode }[] = [
  { dur: 72, render: () => <IntroScene /> },
  {
    dur: 108,
    render: (dur) => (
      <FeatureScene
        kicker="VIDEO LIBRARY"
        kickerIcon={<SearchIcon size={20} />}
        title="Your entire library, indexed"
        bullets={[
          { text: "Recursively scan any folder tree" },
          { text: "Auto-generated thumbnails" },
          { text: "Resolution, codec, FPS & orientation via FFmpeg" },
        ]}
        src="main-screen.png"
        windowLabel="Video Manager  —  Library"
        durationInFrames={dur}
        focusX={50}
        focusY={42}
      />
    ),
  },
  {
    dur: 116,
    render: (dur) => (
      <FeatureScene
        reverse
        kicker="AI SOCIAL COPY"
        kickerIcon={<SparklesIcon size={20} />}
        title="One-click titles, captions & hashtags"
        bullets={[
          { text: "Punchy thumbnail-text ideas" },
          { text: "Instagram, TikTok & YouTube copy" },
          { text: "Written from the video's transcript" },
        ]}
        src="video-ai.png"
        windowLabel="Video Manager  —  AI copy"
        durationInFrames={dur}
        focusX={50}
        focusY={32}
      />
    ),
  },
  {
    dur: 100,
    render: (dur) => (
      <FeatureScene
        kicker="PRODUCTIONS"
        kickerIcon={<LayersIcon size={20} />}
        title="Track every production"
        bullets={[
          { text: "YouTube, TikTok, Instagram & more" },
          { text: "Link clips many-to-many" },
          { text: "Draft / published status" },
        ]}
        src="production.png"
        windowLabel="Video Manager  —  Productions"
        durationInFrames={dur}
        focusX={50}
        focusY={42}
      />
    ),
  },
  { dur: 150, render: () => <PipelineScene /> },
  {
    dur: 128,
    render: (dur) => (
      <FeatureScene
        reverse
        kicker="EDIT & CREATE"
        kickerIcon={<FilmIcon size={20} />}
        title="From raw takes to a finished cut"
        bullets={[
          { text: "Interactive CapCut-style timeline" },
          { text: "Ducked music & burned-in captions" },
          { text: "SEO YouTube copy, generated" },
        ]}
        src="edit-pipeline.png"
        windowLabel="Edit & Create Video"
        durationInFrames={dur}
        focusX={50}
        focusY={36}
      />
    ),
  },
  {
    dur: 106,
    render: (dur) => (
      <FeatureScene
        kicker="BRING YOUR OWN AI"
        kickerIcon={<KeyIcon size={20} />}
        title="Your keys, your models"
        bullets={[
          { text: "ElevenLabs / OpenAI / Gemini transcription" },
          { text: "Gemini / OpenAI / Anthropic LLMs" },
          { text: "Editable prompts — stored locally" },
        ]}
        src="settings.png"
        windowLabel="Video Manager  —  Settings"
        durationInFrames={dur}
        focusX={50}
        focusY={40}
      />
    ),
  },
  { dur: 88, render: () => <OutroScene /> },
];

// Start frames: scene i begins FADE frames before scene i-1 ends.
const STARTS = SCENES.reduce<number[]>((acc, s, i) => {
  acc.push(i === 0 ? 0 : acc[i - 1] + SCENES[i - 1].dur - FADE);
  return acc;
}, []);

export const TOTAL_DURATION =
  STARTS[STARTS.length - 1] + SCENES[SCENES.length - 1].dur;

export const Demo: React.FC = () => {
  return (
    <AbsoluteFill style={{ backgroundColor: theme.bgDeep, fontFamily: theme.font }}>
      <Background />
      {SCENES.map((s, i) => (
        <Sequence key={i} from={STARTS[i]} durationInFrames={s.dur}>
          <SceneFade durationInFrames={s.dur}>{s.render(s.dur)}</SceneFade>
        </Sequence>
      ))}
    </AbsoluteFill>
  );
};
