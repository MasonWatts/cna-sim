import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// The generated ts-rs bindings live outside `web/` (in the Rust crate), so alias them and allow
// Vite's dev server to read one level up. Keeps the game.json schema single-sourced in Rust.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@bindings": path.resolve(__dirname, "../crates/replay/bindings"),
    },
  },
  server: {
    fs: { allow: [".."] },
  },
});
