import { memo } from "react";
import { useIcon } from "../hooks/useIcon";

const SIZE = 20;

/** The app's real icon, or a monogram tile while it loads / when there is none. */
export const AppIcon = memo(function AppIcon({ exe, name }: { exe: string | null; name: string }) {
  const url = useIcon(exe);
  if (url) {
    return <img src={url} alt="" width={SIZE} height={SIZE} className="shrink-0" draggable={false} />;
  }
  return (
    <span
      aria-hidden
      className="flex shrink-0 items-center justify-center rounded-[5px] bg-track text-[10px] font-semibold text-ink-2"
      style={{ width: SIZE, height: SIZE }}
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
});
