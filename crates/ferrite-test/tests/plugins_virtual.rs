//! Vite compat: plugin ordering and virtual modules.

use ferrite::plugin::Apply;
use ferrite::plugin::Enforce;
use ferrite::plugin::Plugin;
use ferrite::plugin::PluginContainer;
use ferrite::plugin::PluginContext;
use ferrite::plugin::TransformRequest as HookTransformRequest;
use ferrite::plugin::TransformResult as HookTransformResult;
use ferrite::server::DevServer;
use ferrite::ModuleId;
use ferrite_test::TempProject;
use std::sync::Arc;
use std::sync::Mutex;

mod common;

use common::dev_config;
use common::dev_server;

// --- plugin ordering -----------------------------------------------------------

struct OrderPlugin {
    name: &'static str,
    enforce: Enforce,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Plugin for OrderPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn enforce(&self) -> Enforce {
        self.enforce
    }

    async fn transform(
        &self,
        _ctx: &PluginContext,
        request: HookTransformRequest,
    ) -> ferrite::Result<Option<HookTransformResult>> {
        self.log.lock().unwrap().push(self.name.to_string());
        Ok(Some(HookTransformResult {
            code: format!("{}// {}\n", request.code, self.name),
            map: None,
            dependencies: Vec::new(),

            module_type: None,
        }))
    }
}

#[tokio::test]
async fn plugin_ordering_pre_normal_post() {
    let project = TempProject::new(&[("src/a.ts", "export const a = 1;\n")]);
    let log = Arc::new(Mutex::new(Vec::new()));
    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(OrderPlugin {
            name: "post",
            enforce: Enforce::Post,
            log: log.clone(),
        }),
        Arc::new(OrderPlugin {
            name: "pre",
            enforce: Enforce::Pre,
            log: log.clone(),
        }),
        Arc::new(OrderPlugin {
            name: "normal",
            enforce: Enforce::Normal,
            log: log.clone(),
        }),
    ];
    let container = PluginContainer::new(plugins.clone(), Apply::All);
    assert_eq!(container.names(), vec!["pre", "normal", "post"]);
    let server = DevServer::new_without_watcher(dev_config(&project), plugins)
        .await
        .unwrap();
    server
        .pipeline_module(&ModuleId::new("/src/a.ts"), None, "client")
        .await
        .unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["pre", "normal", "post"]);
}

// --- virtual modules -----------------------------------------------------------

#[tokio::test]
async fn virtual_modules_roundtrip() {
    let project = TempProject::new(&[
        (".env", "FERRITE_KEY=abc\n"),
        (
            "src/main.ts",
            "import env from \"virtual:ferrite/env\";\nconsole.log(env);\n",
        ),
    ]);
    let server = dev_server(&project).await;
    // Resolver maps to the internal id.
    let resolved = server
        .resolve_entry("virtual:ferrite/env", "client")
        .await
        .unwrap();
    assert_eq!(resolved.0, "\0virtual:ferrite/env");
    // Rewriting uses the servable /@id/ URL.
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(
        main.code.contains("/@id/virtual:ferrite/env"),
        "{}",
        main.code
    );
    // The virtual module serves env content.
    let virtual_module = server
        .pipeline_module(&ModuleId::new("/@id/virtual:ferrite/env"), None, "client")
        .await
        .unwrap();
    assert!(
        virtual_module.code.contains("FERRITE_KEY"),
        "{}",
        virtual_module.code
    );
}
