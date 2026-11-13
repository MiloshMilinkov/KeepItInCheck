// Small inline icon set (stroke icons, 16 px grid). Inline SVG keeps the bundle and the
// network footprint at zero.

type P = { className?: string; size?: number };

const base = (size = 16) => ({
  width: size,
  height: size,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
});

export const IconPause = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M5.5 3.5v9M10.5 3.5v9" />
  </svg>
);
export const IconPlay = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M5 3.5l7 4.5-7 4.5z" />
  </svg>
);
export const IconGear = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="8" cy="8" r="2.2" />
    <path d="M8 1.8v1.6M8 12.6v1.6M14.2 8h-1.6M3.4 8H1.8M12.4 3.6l-1.1 1.1M4.7 11.3l-1.1 1.1M12.4 12.4l-1.1-1.1M4.7 4.7L3.6 3.6" />
  </svg>
);
export const IconCompact = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <rect x="3" y="2.5" width="10" height="11" rx="2" />
    <path d="M3 8h10" />
  </svg>
);
export const IconExpand = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <rect x="3" y="2.5" width="10" height="11" rx="2" />
    <path d="M6 6h4M6 8.5h4M6 11h2" />
  </svg>
);
export const IconHide = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M6 4l4 4-4 4" />
    <path d="M13 3v10" />
  </svg>
);
export const IconChevron = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M6 4l4 4-4 4" />
  </svg>
);
export const IconShield = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M8 1.8l5 2v4c0 3-2.2 5.2-5 6.4-2.8-1.2-5-3.4-5-6.4v-4z" />
  </svg>
);
export const IconCheck = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M3.5 8.5l3 3 6-7" />
  </svg>
);
export const IconClose = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M4 4l8 8M12 4l-8 8" />
  </svg>
);
export const IconRelaunch = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M12.5 8a4.5 4.5 0 1 1-1.3-3.2" />
    <path d="M12.5 2.5v3h-3" />
  </svg>
);
export const IconWarn = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M8 2.5l6 10.5H2z" />
    <path d="M8 6.5v3M8 11.3v.2" />
  </svg>
);
export const IconDots = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M3.5 8h.01M8 8h.01M12.5 8h.01" strokeWidth="2.2" />
  </svg>
);
export const IconBan = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <circle cx="8" cy="8" r="5.5" />
    <path d="M4.2 11.8l7.6-7.6" />
  </svg>
);
export const IconBack = ({ className, size }: P) => (
  <svg {...base(size)} className={className}>
    <path d="M10 4L6 8l4 4" />
  </svg>
);
