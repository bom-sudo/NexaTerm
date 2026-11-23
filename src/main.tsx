import ReactDOM from "react-dom/client";
import "./styles/index.css";
import { App } from "./app/App";

// No React.StrictMode: its dev-only double mount would open every SSH connection twice
// (and show host-key prompts twice). Effects are written to be idempotent regardless.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
