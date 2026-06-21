import React from "react";
import {
  AbsoluteFill,
  Img,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { theme } from "../theme";
import { GithubIcon, StarIcon } from "../components/Icons";

const STACK = ["Rust", "Tauri", "Next.js", "FFmpeg", "SQLite"];

export const OutroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const logo = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 32 });
  const stack = spring({ frame: frame - 14, fps, config: { damping: 200 }, durationInFrames: 28 });
  const cta = spring({ frame: frame - 24, fps, config: { damping: 200 }, durationInFrames: 28 });

  return (
    <AbsoluteFill
      style={{
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
      }}
    >
      <Img
        src={staticFile("Logo.png")}
        style={{
          width: 820,
          opacity: logo,
          transform: `scale(${interpolate(logo, [0, 1], [0.86, 1])})`,
          filter: "drop-shadow(0 18px 50px rgba(59,130,246,0.35))",
        }}
      />

      <div
        style={{
          opacity: stack,
          transform: `translateY(${interpolate(stack, [0, 1], [18, 0])}px)`,
          display: "flex",
          gap: 14,
          marginTop: 8,
        }}
      >
        {STACK.map((s) => (
          <span
            key={s}
            style={{
              padding: "9px 20px",
              borderRadius: 999,
              background: "rgba(255,255,255,0.05)",
              border: `1px solid ${theme.panelBorder}`,
              color: theme.textDim,
              fontSize: 22,
              fontWeight: 600,
            }}
          >
            {s}
          </span>
        ))}
      </div>

      <div
        style={{
          opacity: cta,
          transform: `translateY(${interpolate(cta, [0, 1], [18, 0])}px)`,
          display: "flex",
          alignItems: "center",
          gap: 14,
          marginTop: 46,
          padding: "16px 30px",
          borderRadius: 14,
          background: "rgba(59,130,246,0.14)",
          border: "1px solid rgba(59,130,246,0.4)",
          color: theme.text,
          fontSize: 28,
          fontWeight: 600,
        }}
      >
        <span style={{ display: "flex", color: theme.text }}>
          <GithubIcon size={30} />
        </span>
        github.com/joaoh82/pacingjosh-video-manager
        <span style={{ display: "flex", color: theme.amber, marginLeft: 6 }}>
          <StarIcon size={28} color={theme.amber} />
        </span>
      </div>
    </AbsoluteFill>
  );
};
