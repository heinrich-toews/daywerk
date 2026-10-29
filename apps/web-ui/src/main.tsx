import { render } from "solid-js/web";
import App from "./App";
import { installKeyboardHandling } from "./keyboard";
import "./index.css";

installKeyboardHandling();

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
render(() => <App />, root);
