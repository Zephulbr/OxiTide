import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // Component tests mount Svelte in jsdom, which needs the client runtime
  // rather than the server build Vitest would otherwise resolve.
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
});
