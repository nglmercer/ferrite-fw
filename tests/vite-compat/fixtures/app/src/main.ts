import { greet } from "./greet";
import "./style.css";

document.querySelector("#app")!.innerHTML = `<p>${greet("fixture")}</p>`;

export async function lazy() {
  return import("./lazy");
}

if (import.meta.hot) {
  import.meta.hot.accept();
}
