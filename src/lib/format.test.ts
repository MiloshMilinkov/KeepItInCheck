import { describe, expect, it } from "vitest";
import { baseName, formatBytes, formatDuration, formatPct, isHigh } from "./format";

const MB = 1024 * 1024;

describe("format", () => {
  it("formats bytes", () => {
    expect(formatBytes(312 * MB)).toBe("312 MB");
    expect(formatBytes(1536 * MB)).toBe("1.5 GB");
    expect(formatBytes(12 * 1024 * MB)).toBe("12 GB");
    expect(formatBytes(800 * 1024)).toBe("800 KB");
    expect(formatBytes(0)).toBe("0 MB");
  });

  it("formats percentages", () => {
    expect(formatPct(0)).toBe("0%");
    expect(formatPct(0.05)).toBe("<0.1%");
    expect(formatPct(4.26)).toBe("4.3%");
    expect(formatPct(42.6)).toBe("43%");
  });

  it("formats durations", () => {
    expect(formatDuration(45)).toBe("45 s");
    expect(formatDuration(47 * 60)).toBe("47 min");
    expect(formatDuration(2 * 3600 + 5 * 60)).toBe("2 h 5 min");
  });

  it("reserves red for high load", () => {
    expect(isHigh(60)).toBe(false);
    expect(isHigh(91)).toBe(true);
  });

  it("extracts file names", () => {
    expect(baseName("C:\\Apps\\Spotify.exe")).toBe("spotify.exe");
    expect(baseName("/usr/bin/node")).toBe("node");
  });
});
