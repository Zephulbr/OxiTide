import { mount } from "svelte";
import App from "./App.svelte";
import { followSystemTheme } from "./theme.svelte";
import { initRemoteLanguage } from "./i18n.svelte";
import "../styles/tokens.css";
import "../styles/modes.css";
import "../styles/themes.css";
import "../styles/fonts.css";
import "./remote.css";
import "../styles/forced-colors.css";

followSystemTheme();

// The phone's language loads before the first frame, so text never flashes
// English (initRemoteLanguage never rejects).
async function boot() {
  await initRemoteLanguage();
  return mount(App, { target: document.getElementById("remote")! });
}

export default boot();
