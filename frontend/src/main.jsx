import { render } from "solid-js/web";
import App from "./App.jsx";
import "./styles.css";

const themeQuery = window.matchMedia?.("(prefers-color-scheme: dark)");

function applyDeviceTheme() {
  const theme = themeQuery?.matches ? "dark" : "light";
  document.documentElement.dataset.theme = theme;
}

applyDeviceTheme();
themeQuery?.addEventListener("change", applyDeviceTheme);

render(() => <App />, document.getElementById("root"));
