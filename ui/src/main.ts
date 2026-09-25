import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/tokens.css";
import "./styles/modes.css";
import "./styles/fonts.css";
import "./app.css";

export default mount(App, { target: document.getElementById("app")! });
