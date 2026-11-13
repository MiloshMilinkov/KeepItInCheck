/*
 * KeepItInCheck
 * Copyright (c) 2026 Miloš Milinkov. All rights reserved.
 * Proprietary and confidential. Use, copying, modification or distribution
 * without written permission is prohibited. See the LICENSE file.
 */

import "@fontsource-variable/geist";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

// Rendered without StrictMode so development behaves exactly like the shipped build.
createRoot(document.getElementById("root")!).render(<App />);

// Block the browser context menu and reload shortcuts: this is an app, not a page.
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "F5" || (e.ctrlKey && e.key.toLowerCase() === "r")) e.preventDefault();
});
