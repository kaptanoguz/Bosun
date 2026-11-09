import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri sabit bir port bekler.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { target: "safari15", chunkSizeWarningLimit: 800 },
});
