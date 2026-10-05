import type { SVGProps } from "react";

type IconName =
  | "ai"
  | "automation"
  | "camera"
  | "check"
  | "chevron"
  | "close"
  | "general"
  | "privacy"
  | "refresh"
  | "sliders";

interface IconProps extends SVGProps<SVGSVGElement> {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 20, ...props }: IconProps) {
  const common = {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.7,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };

  const paths: Record<IconName, React.ReactNode> = {
    general: (
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-1.86 1.86-.06-.06A1.7 1.7 0 0 0 16 18.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V20h-5.2v-.1A1.7 1.7 0 0 0 8 18.4a1.7 1.7 0 0 0-1.88.34l-.06.06-1.86-1.86.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.6-1H3v-4h.1A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06L6.06 5.2l.06.06A1.7 1.7 0 0 0 8 5.6a1.7 1.7 0 0 0 1.4-1.5V4h5.2v.1A1.7 1.7 0 0 0 16 5.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 1.86 1.86-.06.06A1.7 1.7 0 0 0 19.4 9a1.7 1.7 0 0 0 1.5 1h.1v4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
      </>
    ),
    ai: (
      <>
        <rect x="7" y="7" width="10" height="10" rx="2" />
        <path d="M9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4M10 10h4v4h-4z" />
      </>
    ),
    automation: <path d="m13 2-8 12h7l-1 8 8-12h-7l1-8Z" />,
    camera: (
      <>
        <path d="M4 7.5h3l1.4-2h7.2l1.4 2h3v11H4z" />
        <circle cx="12" cy="13" r="3.2" />
      </>
    ),
    privacy: <path d="M12 3 5 6v5c0 4.6 2.9 8 7 10 4.1-2 7-5.4 7-10V6l-7-3Z" />,
    refresh: <path d="M20 7v5h-5M4 17v-5h5M6.1 9a7 7 0 0 1 11.2-2L20 10M4 14l2.7 3a7 7 0 0 0 11.2-2" />,
    sliders: (
      <>
        <path d="M4 6h6M14 6h6M4 12h10M18 12h2M4 18h2M10 18h10" />
        <circle cx="12" cy="6" r="2" />
        <circle cx="16" cy="12" r="2" />
        <circle cx="8" cy="18" r="2" />
      </>
    ),
    close: <path d="m7 7 10 10M17 7 7 17" />,
    check: <path d="m5 12 4 4L19 6" />,
    chevron: <path d="m9 6 6 6-6 6" />,
  };

  return (
    <svg {...common} {...props}>
      {paths[name]}
    </svg>
  );
}

export function BrandMark({ size = 32 }: { size?: number }) {
  return (
    <svg
      aria-hidden="true"
      className="brand-mark"
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
    >
      <path d="M16 2.5c1.3 7.2 3.1 9 10.3 10.3-7.2 1.3-9 3.1-10.3 10.3-1.3-7.2-3.1-9-10.3-10.3C12.9 11.5 14.7 9.7 16 2.5Z" fill="currentColor" />
      <path d="M25.3 21.4c.5 2.8 1.2 3.5 4 4-2.8.5-3.5 1.2-4 4-.5-2.8-1.2-3.5-4-4 2.8-.5 3.5-1.2 4-4Z" fill="currentColor" opacity=".55" />
    </svg>
  );
}
