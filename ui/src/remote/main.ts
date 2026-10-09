import { mount } from "svelte";
import App from "./App.svelte";
import "../styles/tokens.css";
import "../styles/modes.css";
import "../styles/themes.css";
import "../styles/fonts.css";
import "./remote.css";

/// Light/dark follows the phone (spec §6.5). Set here, not inline: the page
/// CSP allows no inline scripts.
function applyTheme(query: MediaQueryList) {
  const theme = query.matches ? "dark" : "light";
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme;
}

const scheme = window.matchMedia("(prefers-color-scheme: dark)");
applyTheme(scheme);
scheme.addEventListener("change", () => applyTheme(scheme));

export default mount(App, { target: document.getElementById("remote")! });
