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

export const IntroScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const logo = spring({ frame, fps, config: { damping: 200 }, durationInFrames: 34 });
  const logoScale = interpolate(logo, [0, 1], [0.82, 1]);

  const tagline = spring({
    frame: frame - 16,
    fps,
    config: { damping: 200 },
    durationInFrames: 30,
  });
  const sub = spring({
    frame: frame - 26,
    fps,
    config: { damping: 200 },
    durationInFrames: 30,
  });

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
          width: 940,
          opacity: logo,
          transform: `scale(${logoScale})`,
          filter: "drop-shadow(0 18px 50px rgba(59,130,246,0.35))",
        }}
      />

      <div
        style={{
          opacity: tagline,
          transform: `translateY(${interpolate(tagline, [0, 1], [22, 0])}px)`,
          marginTop: 10,
          fontSize: 40,
          fontWeight: 700,
          color: theme.text,
          letterSpacing: -0.5,
        }}
      >
        Index. Search.{" "}
        <span style={{ color: theme.accentLight }}>Edit with AI.</span>
      </div>

      <div
        style={{
          opacity: sub,
          transform: `translateY(${interpolate(sub, [0, 1], [18, 0])}px)`,
          marginTop: 18,
          fontSize: 25,
          fontWeight: 500,
          color: theme.textDim,
        }}
      >
        A local-first desktop app for your whole video library
      </div>
    </AbsoluteFill>
  );
};
