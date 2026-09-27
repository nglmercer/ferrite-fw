import { greet } from "./greet";
import "./style.css";

const el = document.querySelector("#app");
if (el) {
  el.innerHTML = `<h1>${greet("ferrite")}</h1><p>mode: ${import.meta.env.MODE}</p>`;
}

if (import.meta.hot) {
  import.meta.hot.accept();
}
