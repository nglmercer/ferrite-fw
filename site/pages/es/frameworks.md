---
title: Frameworks
order: 12
---

# Frameworks

Compilar componentes, ejecutarlos en el navegador y renderizar SSR son capacidades
distintas. El [registro de capacidades](../../../crates/ferrite-frameworks/src/registry.rs)
es la referencia para generación y diagnóstico. Todos los perfiles de aplicaciones
siguen siendo experimentales.

| Framework | Compilación del cliente | Host de compilación | Perfil SSR |
|---|---|---|---|
| React 19.2.0 | Oxc/SWC nativo; React Refresh parcial | Nativo | No disponible |
| Vue 3.5.22 | Compilador oficial de la versión del proyecto | Node explícito | napi-vm experimental |
| Svelte 5.39.6 | Compilador oficial de la versión del proyecto | Node explícito | Pendiente de aceptación completa del renderizador |

Vue y Svelte usan compiladores oficiales reales, sin plantillas simuladas. Si no
se configura explícitamente el host, compilar componentes falla con instrucciones.
Node se necesita para compilar estos perfiles durante desarrollo y build; es una
dependencia separada del motor SSR integrado y del ejecutable distribuido.

## Crear aplicaciones

```bash
ferrite create my-react --framework react --language ts --rendering client
ferrite create my-vue --framework vue --language ts --rendering client --compiler-host node
ferrite create my-svelte --framework svelte --language js --rendering client --compiler-host node
```

React Refresh tiene pruebas reales para componentes, hooks, reexportaciones,
`memo`, exportaciones por defecto, recuperación de errores y preview de producción.
La matriz completa sigue pendiente. Producción elimina la instrumentación Refresh.
Detectar Preact, Solid o Qwik no habilita adaptadores para ellos.

Vue/Svelte verifican plantillas e interacción en Chromium y Firefox. El compilador,
sus dependencias, el host y los archivos utilizados participan en la caché y su
invalidación. Los cambios pueden recargar el documento; HMR granular con
conservación del estado todavía no es un contrato completo.

## Vue SSR experimental

Compila Ferrite con `--features napi-vm` y selecciona ambos hosts:

```bash
ferrite create my-app --framework vue --language ts --rendering ssr --compiler-host node --ssr-runtime napi-vm
```

Las pruebas JS/TS cubren HTML HTTP antes de JavaScript, hidratación, interacción,
cambios observados, recuperación de errores de sintaxis/CSS y preview, también bajo
`/app/`. No demuestran streaming, SSG, todas las funciones Vue ni hidratación de
binarios en todas las plataformas. SSR React/Svelte permanece no disponible hasta
superar sus pruebas completas.

`ferrite doctor` muestra requisitos configurados; no ejecuta los compiladores.
`ferrite inspect` muestra el perfil resuelto. Consulta el
[plan de finalización](../../../docs/COMPLETION_PLAN.md) para las tareas y comandos.
