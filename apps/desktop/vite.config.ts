import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri dev server: fixed port, no screen clearing so cargo output stays visible.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "safari16", minify: true, sourcemap: false },
});
