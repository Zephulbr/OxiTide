import { mount } from "svelte";
import App from "./App.svelte";
import { followSystemTheme } from "./theme.svelte";
import "../styles/tokens.css";
import "../styles/modes.css";
import "../styles/themes.css";
import "../styles/fonts.css";
import "./remote.css";

followSystemTheme();

export default mount(App, { target: document.getElementById("remote")! });
