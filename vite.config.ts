/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and must not clear its own logs.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    // The app ships inside WebView2 (Chromium), so modern output is safe and smaller.
    cssMinify: true,
    reportCompressedSize: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
