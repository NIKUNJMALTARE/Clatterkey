import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/app.css";

const root = document.getElementById("root");
if (!root) {
	throw new Error("Missing #root element in index.html");
}

// Whether the keyboard or the pointer was used last, for the pack list's
// focus ring (see app.css). Nothing about the key itself is kept.
const page = document.documentElement;
window.addEventListener(
	"keydown",
	() => page.setAttribute("data-keyboard", ""),
	true,
);
window.addEventListener(
	"pointerdown",
	() => page.removeAttribute("data-keyboard"),
	true,
);

createRoot(root).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
