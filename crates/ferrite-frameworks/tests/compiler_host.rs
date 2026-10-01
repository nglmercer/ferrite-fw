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
