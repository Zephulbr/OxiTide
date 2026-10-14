import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/tokens.css";
import "./styles/modes.css";
import "./styles/themes.css";
import "./styles/fonts.css";
import "./app.css";
// Last, so Rack's [data-mode] rewrites win over the light theme's tokens.
import "./styles/rack.css";

export default mount(App, { target: document.getElementById("app")! });
