//! Create command.

use crate::cli::*;
use std::path::PathBuf;

pub(crate) async fn create(args: CreateArgs) -> ferrite::Result<()> {
    let dir = PathBuf::from(&args.name);
    if dir.exists() {
        return Err(ferrite::FerriteError::Other(format!(
            "`{}` already exists",
            dir.display()
        )));
    }
    let (main_ts, extra) = match args.template.as_str() {
        "ssr" => (
            "export function render(url: string): string {\n  return `<h1>hello from ${url}</h1>`;\n}\n",
            Some(("src/entry-server.ts", "export { render } from \"./main\";\n")),
        ),
        _ => (
            "import \"./style.css\";\n\ndocument.querySelector(\"#app\")!.innerHTML = `<h1>hello ferrite</h1>`;\n",
            None,
        ),
    };
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::create_dir_all(dir.join("public"))?;
    std::fs::write(dir.join("ferrite.toml"), "[server]\nport = 5173\n")?;
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>ferrite app</title>\n</head>\n<body>\n<div id=\"app\"></div>\n<script type=\"module\" src=\"/src/main.ts\"></script>\n</body>\n</html>\n",
    )?;
    std::fs::write(dir.join("src/main.ts"), main_ts)?;
    std::fs::write(
        dir.join("src/style.css"),
        "body { font-family: system-ui; }\n",
    )?;
    if let Some((path, contents)) = extra {
        std::fs::write(dir.join(path), contents)?;
    }
    println!("created {} (template: {})", dir.display(), args.template);
    println!("  cd {}", dir.display());
    println!("  ferrite dev");
    Ok(())
}
