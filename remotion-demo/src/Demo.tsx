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
  LayersIcon,
  RescanIcon,
  SearchIcon,
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

// Each scene starts FADE frames before the previous ends -> cross-fade.
const D = {
  intro: 78,
  library: 120,
  scanning: 108,
  modal: 120,
  productions: 104,
  pipeline: 168,
  outro: 96,
};

const at = {
  intro: 0,
  library: 78 - FADE,
  scanning: 78 - FADE + 120 - FADE,
  modal: 78 - FADE + 120 - FADE + 108 - FADE,
  productions: 78 - FADE + 120 - FADE + 108 - FADE + 120 - FADE,
  pipeline: 78 - FADE + 120 - FADE + 108 - FADE + 120 - FADE + 104 - FADE,
  outro: 78 - FADE + 120 - FADE + 108 - FADE + 120 - FADE + 104 - FADE + 168 - FADE,
};

export const TOTAL_DURATION = at.outro + D.outro; // keep Root in sync

export const Demo: React.FC = () => {
  return (
    <AbsoluteFill style={{ backgroundColor: theme.bgDeep, fontFamily: theme.font }}>
      <Background />

      <Sequence from={at.intro} durationInFrames={D.intro}>
        <SceneFade durationInFrames={D.intro}>
          <IntroScene />
        </SceneFade>
      </Sequence>

      <Sequence from={at.library} durationInFrames={D.library}>
        <SceneFade durationInFrames={D.library}>
          <FeatureScene
            kicker="VIDEO LIBRARY"
            kickerIcon={<SearchIcon size={20} />}
            title="Your entire library, indexed"
            bullets={[
              { text: "Recursive scan of any folder tree" },
              { text: "Auto-generated thumbnails" },
              { text: "Duration, resolution, codec & FPS via FFmpeg" },
            ]}
            src="main-screen.png"
            windowLabel="Video Manager  —  Library"
            durationInFrames={D.library}
            focusX={55}
            focusY={42}
          />
        </SceneFade>
      </Sequence>

      <Sequence from={at.scanning} durationInFrames={D.scanning}>
        <SceneFade durationInFrames={D.scanning}>
          <FeatureScene
            reverse
            kicker="FAST INDEXING"
            kickerIcon={<RescanIcon size={20} />}
            title="Scan thousands of clips"
            bullets={[
              { text: "Live progress with ETA" },
              { text: "Per-file status as it runs" },
              { text: "Rescan to pick up new footage" },
            ]}
            src="scanning.png"
            windowLabel="Video Manager  —  Scanning…"
            durationInFrames={D.scanning}
            focusX={40}
            focusY={20}
          />
        </SceneFade>
      </Sequence>

      <Sequence from={at.modal} durationInFrames={D.modal}>
        <SceneFade durationInFrames={D.modal}>
          <FeatureScene
            kicker="BROWSE & TAG"
            kickerIcon={<FilmIcon size={20} />}
            title="Play, tag & edit in place"
            bullets={[
              { text: "Built-in player with seeking" },
              { text: "Inline metadata & notes" },
              { text: "Multi-tag and categories" },
            ]}
            src="video-modal.png"
            windowLabel="GRDS1843361471542.mp4"
            durationInFrames={D.modal}
            focusX={40}
            focusY={45}
          />
        </SceneFade>
      </Sequence>

      <Sequence from={at.productions} durationInFrames={D.productions}>
        <SceneFade durationInFrames={D.productions}>
          <FeatureScene
            reverse
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
            durationInFrames={D.productions}
            focusX={50}
            focusY={40}
          />
        </SceneFade>
      </Sequence>

      <Sequence from={at.pipeline} durationInFrames={D.pipeline}>
        <SceneFade durationInFrames={D.pipeline}>
          <PipelineScene />
        </SceneFade>
      </Sequence>

      <Sequence from={at.outro} durationInFrames={D.outro}>
        <SceneFade durationInFrames={D.outro}>
          <OutroScene />
        </SceneFade>
      </Sequence>
    </AbsoluteFill>
  );
};
