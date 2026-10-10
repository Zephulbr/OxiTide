import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      // The desktop shell plus the phone remote page (Track 11), served by
      // the app's LAN server from the same embedded dist.
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        remote: fileURLToPath(new URL("./remote.html", import.meta.url)),
      },
    },
  },
  // Component tests mount Svelte in jsdom, which needs the client runtime
  // rather than the server build Vitest would otherwise resolve.
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
});
