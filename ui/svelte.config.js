import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// Vitest 2 runs its own nested Vite 5, and Vite 6's `preprocessCSS` crashes on
// that config. Component styles are plain CSS, so tests skip style
// preprocessing; the app build is unchanged.
export default { preprocess: vitePreprocess({ style: !process.env.VITEST }) };
