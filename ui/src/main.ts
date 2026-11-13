import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/tokens.css";
import "./styles/modes.css";
import "./styles/themes.css";
import "./styles/fonts.css";
import "./app.css";
// Last, so Rack's [data-mode] rewrites win over the light theme's tokens.
import "./styles/rack.css";
import { installNativeMenuSuppression } from "./lib/nativeContextMenu";
import { i18n } from "./lib/i18n.svelte";
import { readLanguageMirror } from "./lib/language.svelte";

installNativeMenuSuppression();

// The last resolved language loads before the first frame, so text never
// flashes in English (spec §4.3); English needs no wait. The config
// reconciles in `appearance.init`.
async function boot() {
  const mirrored = readLanguageMirror();
  if (mirrored !== "en") await i18n.setLanguage(mirrored);
  document.documentElement.lang = i18n.locale;
  return mount(App, { target: document.getElementById("app")! });
}

export default boot();
