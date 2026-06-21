import React from "react";
import {
  Img,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";
import { theme } from "../theme";

/**
 * A macOS-style window chrome that frames an app screenshot, with a spring
 * entrance and a slow Ken Burns zoom so static stills feel alive.
 */
export const Window: React.FC<{
  src: string;
  label: string;
  durationInFrames: number;
  focusX?: number;
  focusY?: number;
}> = ({ src, label, durationInFrames, focusX = 50, focusY = 50 }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const enter = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 28 });
  const scaleIn = interpolate(enter, [0, 1], [0.93, 1]);
  const translateY = interpolate(enter, [0, 1], [40, 0]);
  const ken = interpolate(frame, [0, durationInFrames], [1.02, 1.1], {
    extrapolateRight: "clamp",
  });

  return (
    <div
      style={{
        width: "100%",
        transform: `translateY(${translateY}px) scale(${scaleIn})`,
        borderRadius: 18,
        overflow: "hidden",
        background: theme.bg,
        border: `1px solid ${theme.panelBorder}`,
        boxShadow:
          "0 40px 90px -25px rgba(0,0,0,0.8), 0 0 0 1px rgba(255,255,255,0.03)",
      }}
    >
      <div
        style={{
          height: 44,
          display: "flex",
          alignItems: "center",
          gap: 9,
          padding: "0 18px",
          background: "rgba(255,255,255,0.04)",
          borderBottom: `1px solid ${theme.panelBorder}`,
        }}
      >
        <span style={{ width: 13, height: 13, borderRadius: "50%", background: "#ff5f57" }} />
        <span style={{ width: 13, height: 13, borderRadius: "50%", background: "#febc2e" }} />
        <span style={{ width: 13, height: 13, borderRadius: "50%", background: "#28c840" }} />
        <div
          style={{
            margin: "0 auto",
            padding: "5px 16px",
            borderRadius: 8,
            background: "rgba(255,255,255,0.05)",
            color: theme.textDim,
            fontSize: 18,
            fontWeight: 500,
            maxWidth: "60%",
            overflow: "hidden",
            whiteSpace: "nowrap",
            textOverflow: "ellipsis",
          }}
        >
          {label}
        </div>
      </div>
      <div
        style={{
          aspectRatio: "16 / 10",
          overflow: "hidden",
          background: theme.bg,
          display: "flex",
        }}
      >
        <Img
          src={staticFile(src)}
          style={{
            width: "100%",
            height: "100%",
            objectFit: "contain",
            transform: `scale(${ken})`,
            transformOrigin: `${focusX}% ${focusY}%`,
          }}
        />
      </div>
    </div>
  );
};
