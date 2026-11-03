import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { startLeaderElection } from "./platform/leader.svelte";
import { registerServiceWorker } from "./platform/serviceWorker.svelte";

mount(App, { target: document.getElementById("app") as HTMLElement });
startLeaderElection();
registerServiceWorker();
