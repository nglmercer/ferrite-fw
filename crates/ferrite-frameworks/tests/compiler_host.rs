//! Actual compiler execution with a Ferrite-installed clean project or explicit fixture.
use ferrite_frameworks::compiler_host::*;
use ferrite_transform::{JsCompiler, OxcCompiler, ParseRequest, TransformRequest};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires real Node and registry access or FERRITE_COMPILER_FIXTURE"]
async fn actual_project_matched_compilers_client_server_and_runes() {
    let clean = tempfile::tempdir().unwrap();
    let root = if let Some(root) = std::env::var_os("FERRITE_COMPILER_FIXTURE") {
        PathBuf::from(root)
    } else {
        let root = clean.path().to_path_buf();
        let npm = root.join(".ferrite/npm");
        let client =
            ferrite_npm::RegistryClient::new(ferrite_npm::DEFAULT_REGISTRY, npm.join("metadata"))
                .unwrap();
        let installer = ferrite_npm::Installer::new(client, npm);
        let manifest = ferrite_npm::JsPackageJson {
            dependencies: std::collections::HashMap::from([
                ("vue".into(), "3.5.22".into()),
                ("svelte".into(), "5.39.6".into()),
            ]),
            ..Default::default()
        };
        manifest.write(&root.join("package.json")).unwrap();
        let mut lock = ferrite_npm::Lockfile::default();
        installer
            .install_manifest(&manifest, &mut lock, false)
            .await
            .unwrap();
        lock.write(&root.join("ferrite.lock")).unwrap();
        root
    };
    let host = NodeCompilerHost::new(
        root.clone(),
        root.join("ferrite.lock"),
        None,
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    let process = host.process_id().unwrap();
    let compiler = OxcCompiler::new(Default::default());
    let vue = "<script setup lang='ts'>import { ref } from 'vue'; const count = ref<number>(0); const color = ref('red');</script><template><section><template v-if='true'><button @click='count++'>{{ count }}</button></template></section></template><style scoped>button { color: v-bind(color); }</style><style module>.label { font-weight: bold; }</style>";
    for (target, development) in [
        (CompileTarget::Client, true),
        (CompileTarget::Server, true),
        (CompileTarget::Client, false),
        (CompileTarget::Server, false),
    ] {
        let result = host
            .compile(CompileRequest {
                framework: Framework::Vue,
                filename: root.join("Counter.vue"),
                source: vue.into(),
                target,
                development,
                module: false,
            })
            .await
            .unwrap();
        assert_eq!(result.compiler_version, "3.5.22");
        assert_eq!(result.module_type, ferrite_core::ModuleType::Ts);
        assert_eq!(result.css.len(), 2);
        assert!(result.css[0].code.contains("data-v-"));
        let css_variable = result.css[0]
            .code
            .split("var(--")
            .nth(1)
            .unwrap()
            .split(')')
            .next()
            .unwrap();
        assert!(
            result.code.contains(css_variable),
            "CSS and script must agree on CSS variable naming for development={development}"
        );
        assert!(result.css[1]
            .modules
            .as_ref()
            .unwrap()
            .get("label")
            .is_some());
        let lowered = compiler
            .transform(TransformRequest::new(
                "/Counter.ts",
                &result.code,
                result.module_type,
            ))
            .unwrap();
        compiler
            .parse(ParseRequest {
                id: "/Counter.mjs".into(),
                code: lowered.code,
                module_type: ferrite_core::ModuleType::Js,
            })
            .unwrap();
        let map = result.map.unwrap().mappings;
        let map = oxc_sourcemap::SourceMap::from_json_string(&map).unwrap();
        assert!(map
            .get_tokens()
            .any(|token| token.get_source_id().is_some()));
        match target {
            CompileTarget::Client => assert!(result.code.contains("__sfc__.render = render")),
            CompileTarget::Server => assert!(result.code.contains("__sfc__.ssrRender = ssrRender")),
        }
    }
    let svelte = "<script lang='ts'>let count: number = $state(0);</script><button onclick={() => count++}>{count}</button><style>button { color: red; }</style>";
    for target in [CompileTarget::Client, CompileTarget::Server] {
        let result = host
            .compile(CompileRequest {
                framework: Framework::Svelte,
                filename: root.join("Counter.svelte"),
                source: svelte.into(),
                target,
                development: false,
                module: false,
            })
            .await
            .unwrap();
        assert_eq!(result.compiler_version, "5.39.6");
        assert!(result.code.contains("svelte/internal"));
        assert_eq!(result.css.len(), 1);
        assert!(result.map.is_some());
        compiler
            .parse(ParseRequest {
                id: "/Counter.mjs".into(),
                code: result.code,
                module_type: result.module_type,
            })
            .unwrap();
    }
    // Svelte 5.39.6's multi-root map includes an impossible negative source
    // column. Keep every valid token, retain the generated position as an
    // unmapped barrier, and report the lost upstream location explicitly.
    let source = "<script>\nlet count = $state(0);\n</script>\n<h1>Hello Ferrite + Svelte</h1><button id=\"counter\" onclick={() => count += 1}>count: {count}</button>\n<style>button { padding: .5rem 1rem; color: rgb(128, 0, 0); }</style>\n";
    let result = host
        .compile(CompileRequest {
            framework: Framework::Svelte,
            filename: root.join("App.svelte"),
            source: source.into(),
            target: CompileTarget::Client,
            development: true,
            module: false,
        })
        .await
        .unwrap();
    assert!(result.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "invalid_original_source_map_position"
        && diagnostic.severity == "warning"));
    // Obtain an independent raw reference from the exact same official package.
    // Its decoder preserves signed positions, including subsequent valid tokens.
    let reference = tokio::process::Command::new("node")
        .args(["-e", "const {createRequire}=require('node:module');const r=createRequire(process.argv[1]+'/package.json');const c=r('svelte/compiler');const {decode}=r('@jridgewell/sourcemap-codec');console.log(JSON.stringify(decode(c.compile(process.argv[3],{filename:process.argv[2],generate:'client',dev:true,css:'external',hmr:false}).js.map.mappings)));" ])
        .arg(root.join(".ferrite/npm/packages/svelte@5.39.6"))
        .arg(root.join("App.svelte"))
        .arg(source)
        .kill_on_drop(true).output();
    let reference = tokio::time::timeout(std::time::Duration::from_secs(10), reference)
        .await
        .unwrap()
        .unwrap();
    assert!(
        reference.status.success(),
        "{}",
        String::from_utf8_lossy(&reference.stderr)
    );
    let rows: Vec<Vec<Vec<i64>>> = serde_json::from_slice(&reference.stdout).unwrap();
    let expected: Vec<_> = rows
        .iter()
        .enumerate()
        .flat_map(|(line, row)| {
            row.iter().map(move |segment| {
                let original = (segment.len() > 1 && segment[2] >= 0 && segment[3] >= 0)
                    .then(|| (segment[1] as u32, segment[2] as u32, segment[3] as u32));
                (line as u32, segment[0] as u32, original)
            })
        })
        .collect();
    let encoded = result.map.unwrap().mappings;
    let actual = oxc_sourcemap::SourceMap::from_json_string(&encoded).unwrap();
    let actual: Vec<_> = actual
        .get_tokens()
        .map(|token| {
            (
                token.get_dst_line(),
                token.get_dst_col(),
                token
                    .get_source_id()
                    .map(|source| (source, token.get_src_line(), token.get_src_col())),
            )
        })
        .collect();
    assert_eq!(actual, expected, "every valid source position and invalid-location barrier must remain exact, including valid positions after an invalid one");
    let result = host.compile(CompileRequest { framework: Framework::Svelte, filename: root.join("counter.svelte.ts"), source: "let count: number = $state(0); export function increment() { count++; return count; }".into(), target: CompileTarget::Client, development: true, module: true }).await.unwrap();
    assert!(result.code.contains("svelte/internal"));
    assert!(!result.code.contains(": number"));
    assert_eq!(result.compiler_version, "5.39.6");
    let map = result.map.unwrap().mappings;
    let map = oxc_sourcemap::SourceMap::from_json_string(&map).unwrap();
    assert!(map
        .get_sources()
        .any(|source| source.ends_with("counter.svelte.ts")));
    assert_eq!(
        host.process_id().unwrap(),
        process,
        "compiler calls share the persistent worker"
    );
    let error = host
        .compile(CompileRequest {
            framework: Framework::Vue,
            filename: root.join("Bad.vue"),
            source: "<template><button></template>".into(),
            target: CompileTarget::Client,
            development: true,
            module: false,
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Bad.vue"));
    for (source, expected) in [
        (
            "<template><p>ok</p></template><style lang='scss'>$color: red;</style>",
            "preprocessor",
        ),
        (
            "<template><p>ok</p></template><docs>custom</docs>",
            "custom blocks",
        ),
    ] {
        let error = host
            .compile(CompileRequest {
                framework: Framework::Vue,
                filename: root.join("Unsupported.vue"),
                source: source.into(),
                target: CompileTarget::Client,
                development: true,
                module: false,
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    host.cancel();
    assert!(host
        .compile(CompileRequest {
            framework: Framework::Svelte,
            filename: root.join("Bad.svelte"),
            source: "<button>ok</button>".into(),
            target: CompileTarget::Client,
            development: false,
            module: false
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("stopped"));
}
