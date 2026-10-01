// Focused official compiler wrappers, hosted by the persistent Node transport.
// This module has no SSR renderer and does not supply a browser runtime.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const compilers = new Map();
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
function errors(values, filename) {
  if (values?.length) throw new Error(`${filename}: ${values.map(value => typeof value === 'string' ? value : value.message).join('\n')}`);
}
export async function compile(request) {
  const module = compiler(request);
  const { filename, source, target, development } = request;
  const server = target === 'server';
  if (request.framework === 'svelte') {
    const result = request.module
      ? module.compileModule(source, { filename, generate: target, dev: development })
      : module.compile(source, { filename, generate: target, dev: development, css: 'external', hmr: false });
    return { pieces: [{ code: result.js.code, map: map(result.js.map) }], language: 'js', css: result.css ? [{ id: 'style-0', code: result.css.code, map: map(result.css.map), modules: null }] : [], dependencies: [], diagnostics: result.warnings.map(warning => ({ severity: 'warning', code: warning.code, message: warning.message, filename: warning.filename || filename, start: warning.start || null })), compilerVersion: module.VERSION };
  }
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
