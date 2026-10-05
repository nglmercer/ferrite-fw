// Focused official compiler wrappers, hosted by the persistent Node transport.
// This module has no SSR renderer and does not supply a browser runtime.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const compilers = new Map();
const vueTypeDependencies = new WeakMap();
function compiler(request) {
  const { framework, packageDir, version } = request;
  const manifest = JSON.parse(readFileSync(join(packageDir, 'package.json'), 'utf8'));
  if (manifest.name !== framework || manifest.version !== version) throw new Error(`compiler package identity mismatch: expected ${framework}@${version}`);
  const key = `${packageDir}@${version}`;
  if (!compilers.has(key)) {
    const require = createRequire(join(packageDir, 'package.json'));
    const module = require(framework === 'vue' ? 'vue/compiler-sfc' : 'svelte/compiler');
    const actual = framework === 'vue' ? module.version : module.VERSION;
    if (actual !== version) throw new Error(`compiler version ${actual} differs from selected project ${framework}@${version}`);
    compilers.set(key, module);
  }
  return compilers.get(key);
}
function map(value) { return value ? JSON.parse(JSON.stringify(value)) : null; }
// ECMA-426 DecodeMappingsField: invalid original positions are null, with
// a reported diagnostic. Never clamp them to fabricated source coordinates.
function svelteMap(value, request, diagnostics) {
  const json = map(value);
  if (!json) return null;
  const require = createRequire(join(request.packageDir, 'package.json'));
  const { decode, encode } = require('@jridgewell/sourcemap-codec');
  const lines = decode(json.mappings);
  let invalid = 0;
  for (const line of lines) {
    for (let i = 0; i < line.length; i++) {
      const segment = line[i];
      if (segment.length > 1 && (segment[2] < 0 || segment[3] < 0)) {
        line[i] = [segment[0]];
        invalid++;
      }
    }
  }
  if (invalid) {
    json.mappings = encode(lines);
    diagnostics.push({ severity: 'warning', code: 'invalid_original_source_map_position',
      message: `Svelte ${request.version} emitted ${invalid} negative original source-map positions; those positions are unmapped per ECMA-426, and valid mappings are retained`,
      filename: request.filename, start: null });
  }
  return json;
}
function errors(values, filename) {
  if (values?.length) throw new Error(values.map(value => {
    const start = typeof value === 'object' && value !== null ? value.loc?.start : null;
    const position = Number.isInteger(start?.line) && start.line > 0
      && Number.isInteger(start?.column) && start.column > 0
      ? `:${start.line}:${start.column}` : '';
    const message = typeof value === 'string' ? value : value.message;
    return `${filename}${position}: ${message}`;
  }).join('\n'));
}
export async function compile(request) {
  const module = compiler(request);
  const { filename, source, target, development } = request;
  const server = target === 'server';
  if (request.framework === 'svelte') {
    const result = request.module
      ? module.compileModule(source, { filename, generate: target, dev: development })
      : module.compile(source, { filename, generate: target, dev: development, css: 'external', hmr: false });
    const diagnostics = result.warnings.map(warning => ({ severity: 'warning', code: warning.code, message: warning.message, filename: warning.filename || filename, start: warning.start || null }));
    const jsMap = svelteMap(result.js.map, request, diagnostics);
    const cssMap = result.css ? svelteMap(result.css.map, request, diagnostics) : null;
    return { pieces: [{ code: result.js.code, map: jsMap }], language: 'js', css: result.css ? [{ id: 'style-0', code: result.css.code, map: cssMap, modules: null }] : [], dependencies: [], diagnostics, compilerVersion: module.VERSION };
  }
  // The official compiler retains imported type scopes independently of Ferrite's
  // graph cache. Invalidate changed inputs before compileScript reuses them.
  const typeInputs = vueTypeDependencies.get(module) || new Map();
  for (const [path, previous] of typeInputs) {
    let current;
    try { current = readFileSync(path, 'utf8'); } catch { current = null; }
    if (current !== previous) {
      if (typeof module.invalidateTypeCache !== 'function') throw new Error(`${filename}: selected Vue compiler cannot invalidate imported type dependencies`);
      module.invalidateTypeCache(path);
      typeInputs.set(path, current);
    }
  }
  vueTypeDependencies.set(module, typeInputs);
  const parsed = module.parse(source, { filename, sourceMap: true });
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
  const script = descriptor.script || descriptor.scriptSetup ? module.compileScript(descriptor, { id, genDefaultAs: '__sfc__', isProd: !development, sourceMap: true, templateOptions: { ssr: server } }) : null;
  for (const path of script?.deps || []) {
    typeInputs.set(path, readFileSync(path, 'utf8'));
  }
  const diagnostics = [];
  const pieces = [{ code: script?.content || 'const __sfc__ = {};', map: map(script?.map) }];
  if (descriptor.template) {
    const template = module.compileTemplate({ source: descriptor.template.content, filename, id, inMap: descriptor.template.map, scoped: descriptor.styles.some(style => style.scoped), isProd: !development, ssr: server, ssrCssVars: descriptor.cssVars, compilerOptions: { bindingMetadata: script?.bindings || {}, expressionPlugins: script?.lang === 'ts' ? ['typescript'] : [], scopeId: descriptor.styles.some(style => style.scoped) ? `data-v-${id}` : undefined } });
    errors(template.errors, filename);
    for (const tip of template.tips || []) diagnostics.push({ severity: 'warning', code: 'vue-template-tip', message: typeof tip === 'string' ? tip : tip.message, filename, start: null });
    pieces.push({ code: template.code, map: map(template.map) });
    pieces.push({ code: server ? '__sfc__.ssrRender = ssrRender;' : '__sfc__.render = render;', map: null });
  }
  const css = [];
  const dependencies = new Set(script?.deps || []);
  const moduleStyles = {};
  for (const [index, style] of descriptor.styles.entries()) {
    if (style.lang && style.lang !== 'css') throw new Error(`${filename}: style language ${style.lang} requires a configured preprocessor`);
    const result = await module.compileStyleAsync({ source: style.content, filename, id: `data-v-${id}`, scoped: style.scoped, isProd: !development, modules: Boolean(style.module), inMap: style.map });
    errors(result.errors, filename);
    for (const dependency of result.dependencies || []) dependencies.add(dependency);
    css.push({ id: `style-${index}`, code: result.code, map: map(result.map), modules: result.modules || null });
    if (style.module) moduleStyles[typeof style.module === 'string' ? style.module : '$style'] = result.modules;
  }
  if (descriptor.styles.some(style => style.scoped)) pieces.push({ code: `__sfc__.__scopeId = ${JSON.stringify(`data-v-${id}`)};`, map: null });
  if (Object.keys(moduleStyles).length) pieces.push({ code: `__sfc__.__cssModules = ${JSON.stringify(moduleStyles)};`, map: null });
  pieces.push({ code: 'export default __sfc__;', map: null });
  return { pieces, language: script?.lang === 'ts' ? 'ts' : 'js', css, dependencies: [...dependencies], diagnostics, compilerVersion: module.version };
}
