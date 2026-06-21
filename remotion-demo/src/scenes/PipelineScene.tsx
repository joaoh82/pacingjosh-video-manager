import React from "react";
import {
  AbsoluteFill,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { theme } from "../theme";
import {
  CaptionsIcon,
  CheckIcon,
  FilmIcon,
  MicIcon,
  MusicIcon,
  ScissorsIcon,
  SparklesIcon,
} from "../components/Icons";

type Stage = {
  icon: React.ReactNode;
  title: string;
  desc: string;
  color: string;
};

const STAGES: Stage[] = [
  { icon: <MicIcon size={34} />, title: "Transcribe", desc: "Every take, word-level timing", color: theme.accentLight },
  { icon: <SparklesIcon size={34} />, title: "Plan the cut", desc: "LLM writes the edit decision list", color: theme.accent2 },
  { icon: <ScissorsIcon size={34} />, title: "Tighten", desc: "Drop silences & filler — jump cuts", color: theme.green },
  { icon: <CaptionsIcon size={34} />, title: "Caption", desc: "Burned-in, re-timed per clip", color: theme.accentLight },
  { icon: <MusicIcon size={34} />, title: "Duck music", desc: "Auto-lower under speech", color: theme.accent2 },
  { icon: <FilmIcon size={34} />, title: "Stitch", desc: "One MP4 + EDL via FFmpeg", color: theme.green },
];

const STAGGER = 14;

export const PipelineScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const titleS = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 26 });
  const subS = spring({ frame: frame - 10, fps, config: { damping: 200 }, durationInFrames: 26 });

  const fill = interpolate(
    frame,
    [14, 14 + (STAGES.length - 1) * STAGGER + 24],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  return (
    <AbsoluteFill
      style={{
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        padding: "0 60px",
      }}
    >
      <div
        style={{
          opacity: titleS,
          transform: `translateY(${interpolate(titleS, [0, 1], [22, 0])}px)`,
          display: "inline-flex",
          alignItems: "center",
          gap: 14,
          fontSize: 56,
          fontWeight: 800,
          color: theme.text,
          letterSpacing: -1,
        }}
      >
        <span style={{ display: "flex", color: theme.accent2 }}>
          <SparklesIcon size={46} />
        </span>
        The AI Edit &amp; Create Pipeline
      </div>

      <div
        style={{
          opacity: subS,
          marginTop: 14,
          fontSize: 26,
          fontWeight: 500,
          color: theme.textDim,
        }}
      >
        Raw takes + your script → one finished, captioned, music-ducked cut
      </div>

      {/* progress rail */}
      <div
        style={{
          width: 1500,
          height: 5,
          borderRadius: 999,
          background: "rgba(255,255,255,0.08)",
          marginTop: 64,
          marginBottom: 40,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            width: `${fill * 100}%`,
            height: "100%",
            background: `linear-gradient(90deg, ${theme.accent}, ${theme.accent2})`,
          }}
        />
      </div>

      <div style={{ display: "flex", alignItems: "stretch", gap: 18 }}>
        {STAGES.map((s, i) => {
          const start = 14 + i * STAGGER;
          const enter = spring({
            frame: frame - start,
            fps,
            config: { damping: 200 },
            durationInFrames: 24,
          });
          const check = spring({
            frame: frame - start - 14,
            fps,
            config: { damping: 200 },
            durationInFrames: 16,
          });
          return (
            <React.Fragment key={i}>
              <div
                style={{
                  width: 232,
                  boxSizing: "border-box",
                  opacity: enter,
                  transform: `translateY(${interpolate(enter, [0, 1], [34, 0])}px) scale(${interpolate(
                    enter,
                    [0, 1],
                    [0.9, 1]
                  )})`,
                  background: theme.panel,
                  border: `1px solid ${theme.panelBorder}`,
                  borderRadius: 18,
                  padding: "26px 22px",
                  position: "relative",
                  boxShadow: "0 24px 50px -28px rgba(0,0,0,0.8)",
                }}
              >
                <div
                  style={{
                    width: 64,
                    height: 64,
                    borderRadius: 16,
                    background: `${s.color}1f`,
                    border: `1px solid ${s.color}55`,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    color: s.color,
                    marginBottom: 18,
                  }}
                >
                  {s.icon}
                </div>
                <div style={{ fontSize: 27, fontWeight: 700, color: theme.text }}>
                  {s.title}
                </div>
                <div
                  style={{
                    marginTop: 8,
                    fontSize: 19,
                    lineHeight: 1.3,
                    color: theme.textDim,
                    fontWeight: 500,
                  }}
                >
                  {s.desc}
                </div>

                <div
                  style={{
                    position: "absolute",
                    top: 18,
                    right: 18,
                    width: 30,
                    height: 30,
                    borderRadius: "50%",
                    background: theme.green,
                    color: "#04210f",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    opacity: check,
                    transform: `scale(${check})`,
                  }}
                >
                  <CheckIcon size={18} strokeWidth={3} />
                </div>

                <div
                  style={{
                    position: "absolute",
                    top: 14,
                    left: 18,
                    fontSize: 16,
                    fontWeight: 700,
                    color: "rgba(255,255,255,0.25)",
                  }}
                >
                  {String(i + 1).padStart(2, "0")}
                </div>
              </div>

              {i < STAGES.length - 1 && (
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    color: theme.textDim,
                    fontSize: 30,
                    opacity: interpolate(enter, [0, 1], [0, 0.5]),
                  }}
                >
                  ›
                </div>
              )}
            </React.Fragment>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};
