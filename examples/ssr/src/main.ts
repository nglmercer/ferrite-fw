import { render } from "./entry-server";

const el = document.querySelector("#app");
if (el) {
  el.innerHTML = render(window.location.pathname);
}
