import { defineConfig } from "vite";

// Vite is used for the settings UI window only.
// The Tauri Rust HTTP server (port 49124) is what serves the production overlays.
// During `tauri dev`, Tauri proxies the settings UI to this dev server.
export default defineConfig({
  root: "src",
  publicDir: "../public",
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
});
