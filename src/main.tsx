/* @refresh reload */
import { render } from "solid-js/web";
import App from "./App";
import { I18nProvider } from "./i18n";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("#root missing");
render(
  () => (
    <I18nProvider>
      <App />
    </I18nProvider>
  ),
  root,
);
