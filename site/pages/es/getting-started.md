---
title: Primeros pasos
order: 2
---

# Primeros pasos

Solo necesitas una toolchain Rust (estable, 1.88+). Node.js es opcional
y solo lo usan los plugins extranjeros de nivel 3.

## Instalar el CLI

```bash
cargo install --path crates/ferrite-cli
ferrite --version
```

O ejecútalo desde el repo sin instalar:

```bash
cargo run -p ferrite-cli -- --help
```

## Tu primer proyecto

```bash
ferrite create my-app
cd my-app
ferrite dev
```

Abre la URL impresa (por defecto `http://127.0.0.1:5173`). Los cambios
en cualquier módulo recargan en caliente; los cambios en Markdown o
estilos reconstruyen el CSS de la página.

Un proyecto mínimo es solo un `index.html` más un script módulo:

```html
<!doctype html>
<html>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.js"></script>
  </body>
</html>
```

```js
// src/main.js
document.getElementById("app").textContent = "Hola, Ferrite.";
```

## El ciclo dev

1. `ferrite dev` — ESM nativo por módulo, HMR por WebSocket.
2. `ferrite build` — bundle de producción hasheado en `dist/`.
3. `ferrite preview` — sirve `dist/` en local para revisar el resultado.

Cuando algo se ve mal, `ferrite compat` ejecuta auto-chequeos en vivo
contra el pipeline real e informa el fallo en voz alta en vez de adivinar.
