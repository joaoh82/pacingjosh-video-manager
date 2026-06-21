import React from "react";
import {
  AbsoluteFill,
  interpolate,
  spring,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { theme } from "../theme";
import { Window } from "../components/Window";
import { CheckIcon } from "../components/Icons";

export type Bullet = { text: string };

// Pure entrance: a spring-driven fade + rise. Kept hook-free so it can be
// called for any number of bullets without breaking the rules of hooks.
const rise = (frame: number, fps: number, delay: number) => {
  const s = spring({
    frame: frame - delay,
    fps,
    config: { damping: 200 },
    durationInFrames: 26,
  });
  return {
    opacity: s,
    transform: `translateY(${interpolate(s, [0, 1], [26, 0])}px)`,
  };
};

export const FeatureScene: React.FC<{
  kicker: string;
  kickerIcon: React.ReactNode;
  title: string;
  bullets: Bullet[];
  src: string;
  windowLabel: string;
  durationInFrames: number;
  reverse?: boolean;
  focusX?: number;
  focusY?: number;
}> = ({
  kicker,
  kickerIcon,
  title,
  bullets,
  src,
  windowLabel,
  durationInFrames,
  reverse = false,
  focusX,
  focusY,
}) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  return (
    <AbsoluteFill
      style={{
        flexDirection: reverse ? "row-reverse" : "row",
        alignItems: "center",
        gap: 72,
        padding: "0 110px",
      }}
    >
      {/* Text column */}
      <div style={{ width: 600, flexShrink: 0 }}>
        <div
          style={{
            ...rise(frame, fps, 2),
            display: "inline-flex",
            alignItems: "center",
            gap: 10,
            padding: "9px 18px",
            borderRadius: 999,
            background: "rgba(59,130,246,0.12)",
            border: "1px solid rgba(59,130,246,0.35)",
            color: theme.accentLight,
            fontSize: 20,
            fontWeight: 700,
            letterSpacing: 2,
          }}
        >
          <span style={{ display: "flex", color: theme.accentLight }}>{kickerIcon}</span>
          {kicker}
        </div>

        <h1
          style={{
            ...rise(frame, fps, 8),
            margin: "26px 0 34px",
            fontSize: 64,
            lineHeight: 1.05,
            fontWeight: 800,
            color: theme.text,
            letterSpacing: -1,
          }}
        >
          {title}
        </h1>

        <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
          {bullets.map((b, i) => (
            <div
              key={i}
              style={{
                ...rise(frame, fps, 18 + i * 7),
                display: "flex",
                alignItems: "center",
                gap: 16,
                fontSize: 27,
                color: theme.textDim,
                fontWeight: 500,
              }}
            >
              <span
                style={{
                  flexShrink: 0,
                  width: 38,
                  height: 38,
                  borderRadius: 10,
                  background: "rgba(52,211,153,0.14)",
                  border: "1px solid rgba(52,211,153,0.3)",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: theme.green,
                }}
              >
                <CheckIcon size={22} />
              </span>
              {b.text}
            </div>
          ))}
        </div>
      </div>

      {/* Screenshot */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <Window
          src={src}
          label={windowLabel}
          durationInFrames={durationInFrames}
          focusX={focusX}
          focusY={focusY}
        />
      </div>
    </AbsoluteFill>
  );
};
