const el = document.querySelector("#app");
if (el) {
  el.innerHTML = `
    <h1>e2e demo</h1>
    <button id="inc">increment</button>
    <span id="count">0</span>
    <input id="name" placeholder="your name" />
    <p id="greeting"></p>
  `;
  const count = document.querySelector("#count");
  document.querySelector("#inc")?.addEventListener("click", () => {
    if (count) count.textContent = String(Number(count.textContent) + 1);
  });
  const name = document.querySelector("#name");
  const greeting = document.querySelector("#greeting");
  name?.addEventListener("input", () => {
    if (greeting && name instanceof HTMLInputElement) {
      greeting.textContent = name.value ? `hello ${name.value}` : "";
    }
  });
}
