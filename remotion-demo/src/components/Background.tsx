import React from "react";
import { AbsoluteFill, interpolate, useCurrentFrame } from "remotion";
import { theme } from "../theme";

/**
 * Continuous animated backdrop shared across every scene so scene
 * cross-fades read as smooth transitions rather than hard cuts.
 */
export const Background: React.FC = () => {
  const frame = useCurrentFrame();

  const blob1x = interpolate(Math.sin(frame / 120), [-1, 1], [12, 36]);
  const blob1y = interpolate(Math.cos(frame / 150), [-1, 1], [6, 32]);
  const blob2x = interpolate(Math.cos(frame / 140), [-1, 1], [62, 88]);
  const blob2y = interpolate(Math.sin(frame / 110), [-1, 1], [58, 82]);

  const mask = "radial-gradient(circle at 50% 42%, black, transparent 82%)";

  return (
    <AbsoluteFill style={{ backgroundColor: theme.bgDeep }}>
      <AbsoluteFill
        style={{
          background: `radial-gradient(58% 58% at ${blob1x}% ${blob1y}%, rgba(59,130,246,0.28), transparent 70%)`,
        }}
      />
      <AbsoluteFill
        style={{
          background: `radial-gradient(52% 52% at ${blob2x}% ${blob2y}%, rgba(34,211,238,0.16), transparent 70%)`,
        }}
      />
      <AbsoluteFill
        style={{
          backgroundImage:
            "linear-gradient(rgba(255,255,255,0.035) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px)",
          backgroundSize: "64px 64px",
          maskImage: mask,
          WebkitMaskImage: mask,
        }}
      />
      <AbsoluteFill style={{ boxShadow: "inset 0 0 420px rgba(0,0,0,0.75)" }} />
    </AbsoluteFill>
  );
};
