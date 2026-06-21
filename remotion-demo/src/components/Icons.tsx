import React from "react";

type IconProps = {
  size?: number;
  color?: string;
  strokeWidth?: number;
};

const Svg: React.FC<IconProps & { children: React.ReactNode; fill?: string }> = ({
  size = 24,
  color = "currentColor",
  strokeWidth = 2,
  fill = "none",
  children,
}) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill={fill}
    stroke={color}
    strokeWidth={strokeWidth}
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    {children}
  </svg>
);

export const CheckIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M20 6 9 17l-5-5" />
  </Svg>
);

export const SearchIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.3-4.3" />
  </Svg>
);

export const RescanIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
    <path d="M3 3v5h5" />
  </Svg>
);

export const FilmIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M7 3v18M17 3v18M3 7.5h4M17 7.5h4M3 12h18M3 16.5h4M17 16.5h4" />
  </Svg>
);

export const LayersIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M12 2 2 7l10 5 10-5z" />
    <path d="m2 17 10 5 10-5" />
    <path d="m2 12 10 5 10-5" />
  </Svg>
);

export const MicIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <path d="M12 19v3" />
  </Svg>
);

export const SparklesIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M9.94 14.06A2 2 0 0 0 8.5 12.62l-5.6-1.45a.5.5 0 0 1 0-.96l5.6-1.45A2 2 0 0 0 9.94 7.3l1.45-5.6a.5.5 0 0 1 .96 0l1.45 5.6a2 2 0 0 0 1.44 1.44l5.6 1.45a.5.5 0 0 1 0 .96l-5.6 1.45a2 2 0 0 0-1.44 1.44l-1.45 5.6a.5.5 0 0 1-.96 0z" />
    <path d="M20 3v4M22 5h-4M4 17v2M5 18H3" />
  </Svg>
);

export const ScissorsIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <circle cx="6" cy="6" r="3" />
    <circle cx="6" cy="18" r="3" />
    <path d="M20 4 8.12 15.88" />
    <path d="M14.47 14.48 20 20" />
    <path d="M8.12 8.12 12 12" />
  </Svg>
);

export const CaptionsIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <rect x="2" y="5" width="20" height="14" rx="2" />
    <path d="M7 15h4M15 15h2M7 11h2M13 11h4" />
  </Svg>
);

export const MusicIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M9 18V5l12-2v13" />
    <circle cx="6" cy="18" r="3" />
    <circle cx="18" cy="16" r="3" />
  </Svg>
);

export const StarIcon: React.FC<IconProps> = (p) => (
  <Svg {...p} fill={p.color ?? "currentColor"} strokeWidth={0}>
    <path d="M11.48 3.5a.6.6 0 0 1 1.04 0l2.3 4.66a.6.6 0 0 0 .45.33l5.14.75a.6.6 0 0 1 .33 1.02l-3.72 3.62a.6.6 0 0 0-.17.53l.88 5.12a.6.6 0 0 1-.87.63l-4.6-2.42a.6.6 0 0 0-.56 0l-4.6 2.42a.6.6 0 0 1-.87-.63l.88-5.12a.6.6 0 0 0-.17-.53L2.26 10.3a.6.6 0 0 1 .33-1.02l5.14-.75a.6.6 0 0 0 .45-.33z" />
  </Svg>
);

export const GithubIcon: React.FC<IconProps> = (p) => (
  <Svg {...p}>
    <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
  </Svg>
);
