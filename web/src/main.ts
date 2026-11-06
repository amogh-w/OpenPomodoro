import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { installDiagnostics } from "./lib/diag.svelte";
import { startLeaderElection } from "./platform/leader.svelte";
import { registerServiceWorker } from "./platform/serviceWorker.svelte";

installDiagnostics();
mount(App, { target: document.getElementById("app") as HTMLElement });
startLeaderElection();
registerServiceWorker();
