// Official Vue compiler wrapper executed inside Ferrite's embedded JS runtime.
// The compiler input is the upstream browser ESM build; this wrapper must not
// use Node builtins.
import * as compiler from '/vue-compiler.mjs';

const typeInputs = new Map();

function clone(value) { return value ? JSON.parse(JSON.stringify(value)) : null; }
function normalize(path) { return String(path).replaceAll('\\', '/'); }
function makeFs(files) {
  const entries = new Map();
  for (const [path, source] of Object.entries(files || {})) entries.set(normalize(path), source);
  return {
    fileExists(path) { return entries.has(normalize(path)); },
    readFile(path) { return entries.get(normalize(path)); },
    realpath(path) { return normalize(path); },
  };
}
function errors(values, filename, blockStart = null) {
  if (values?.length) throw new Error(values.map(value => {
    let start = typeof value === 'object' && value !== null ? value.loc?.start : null;
    if (blockStart && Number.isInteger(value?.line) && Number.isInteger(value?.column)) {
      start = { line: value.line, column: value.column + (value.line === blockStart.line ? blockStart.column - 1 : 0) };
    }
    const position = Number.isInteger(start?.line) && start.line > 0 && Number.isInteger(start?.column) && start.column > 0
      ? `:${start.line}:${start.column}` : '';
    const message = typeof value === 'string' ? value : blockStart && value.reason ? value.reason : value.message;
    return `${filename}${position}: ${message}`;
  }).join('\n'));
}
// HMR signatures only need stable equality within one development session.
// This avoids embedding a cryptographic dependency in the runtime.
function signatureHash(text) {
  let high = 0x811c9dc5;
  let low = 0x811c9dc5;
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    low = Math.imul(low ^ (code & 0xff), 16777619);
    high = Math.imul(high ^ (code >>> 8), 16777619);
  }
  return `${(high >>> 0).toString(16)}${(low >>> 0).toString(16)}`;
}

export async function compile(request) {
  const { filename, source, target, development } = request;
  const server = target === 'server';
  const parsed = compiler.parse(source, { filename, sourceMap: true });
  errors(parsed.errors, filename);
  const descriptor = parsed.descriptor;
  if (descriptor.customBlocks.length) throw new Error(`${filename}: custom blocks require an explicitly registered integration`);
  for (const block of [descriptor.script, descriptor.scriptSetup, descriptor.template, ...descriptor.styles].filter(Boolean)) {
    if (block.src) throw new Error(`${filename}: external SFC blocks require an explicit dependency/map integration`);
  }
  for (const block of [descriptor.script, descriptor.scriptSetup].filter(Boolean)) {
    if (block.lang && !['js', 'ts'].includes(block.lang)) throw new Error(`${filename}: unsupported script language ${block.lang}; configure an explicit preprocessor`);
  }
  if (descriptor.template?.lang && descriptor.template.lang !== 'html') throw new Error(`${filename}: template language ${descriptor.template.lang} requires a configured preprocessor`);
  const id = request.scopeId;
  const fs = makeFs(request.files);
  for (const [path, previous] of typeInputs) {
    const current = request.files?.[path] ?? null;
    if (current !== previous) {
      compiler.invalidateTypeCache(path);
      typeInputs.set(path, current);
    }
  }
  const script = descriptor.script || descriptor.scriptSetup ? compiler.compileScript(descriptor, { id, genDefaultAs: '__sfc__', isProd: !development, sourceMap: true, fs, templateOptions: { ssr: server } }) : null;
  for (const path of script?.deps || []) typeInputs.set(normalize(path), request.files?.[normalize(path)] ?? null);
  const diagnostics = [];
  const pieces = [{ code: script?.content || 'const __sfc__ = {};', map: clone(script?.map) }];
  if (descriptor.template) {
    const template = compiler.compileTemplate({ source: descriptor.template.content, filename, id, inMap: descriptor.template.map, scoped: descriptor.styles.some(style => style.scoped), isProd: !development, ssr: server, ssrCssVars: descriptor.cssVars, compilerOptions: { bindingMetadata: script?.bindings || {}, expressionPlugins: script?.lang === 'ts' ? ['typescript'] : [], scopeId: descriptor.styles.some(style => style.scoped) ? `data-v-${id}` : undefined } });
    errors(template.errors, filename);
    for (const tip of template.tips || []) diagnostics.push({ severity: 'warning', code: 'vue-template-tip', message: typeof tip === 'string' ? tip : tip.message, filename, start: null });
    pieces.push({ code: template.code, map: clone(template.map) });
    pieces.push({ code: server ? '__sfc__.ssrRender = ssrRender;' : '__sfc__.render = render;', map: null });
  }
  const css = [];
  const dependencies = new Set(script?.deps || []);
  const moduleStyles = Object.create(null);
  for (const [index, style] of descriptor.styles.entries()) {
    if (style.lang && style.lang !== 'css') throw new Error(`${filename}: style language ${style.lang} requires a configured preprocessor`);
    const result = await compiler.compileStyleAsync({ source: style.content, filename, id: `data-v-${id}`, scoped: style.scoped, isProd: !development, modules: Boolean(style.module), inMap: style.map });
    errors(result.errors, filename, style.loc.start);
    for (const dependency of result.dependencies || []) dependencies.add(dependency);
    css.push({ id: `style-${index}`, code: result.code, map: clone(result.map), modules: result.modules || null });
    if (style.module) moduleStyles[typeof style.module === 'string' ? style.module : '$style'] = result.modules;
  }
  if (descriptor.styles.some(style => style.scoped)) pieces.push({ code: `__sfc__.__scopeId = ${JSON.stringify(`data-v-${id}`)};`, map: null });
  if (Object.keys(moduleStyles).length) pieces.push({ code: `__sfc__.__cssModules = JSON.parse(${JSON.stringify(JSON.stringify(moduleStyles))});`, map: null });
  if (development && !server) {
    const signature = signatureHash(JSON.stringify([script?.content || '', moduleStyles, descriptor.styles.some(style => style.scoped)]));
    pieces.push({ code: `__sfc__.__hmrId = ${JSON.stringify(id)};
__sfc__.__ferriteHmrSignature = ${JSON.stringify(signature)};
if (import.meta.hot) {
  const runtime = globalThis.__VUE_HMR_RUNTIME__;
  if (!runtime) throw new Error('Vue development runtime does not expose HMR');
  runtime.createRecord(__sfc__.__hmrId, __sfc__);
  import.meta.hot.accept(next => {
    if (!next) return;
    if (next.default.__ferriteHmrSignature === __sfc__.__ferriteHmrSignature) runtime.rerender(__sfc__.__hmrId, next.default.render);
    else runtime.reload(__sfc__.__hmrId, next.default);
  });
}`, map: null });
  }
  pieces.push({ code: 'export default __sfc__;', map: null });
  return { pieces, language: script?.lang === 'ts' ? 'ts' : 'js', css, dependencies: [...dependencies], diagnostics, compilerVersion: compiler.version };
}
