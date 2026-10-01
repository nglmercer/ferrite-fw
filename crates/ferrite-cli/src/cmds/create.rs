//! Create command.

use crate::cli::*;
use std::path::PathBuf;

pub(crate) async fn create(args: CreateArgs) -> ferrite::Result<()> {
    validate_template(&args.template)?;
    let dir = PathBuf::from(&args.name);
    if dir.exists() {
        return Err(ferrite::FerriteError::Other(format!(
            "`{}` already exists",
            dir.display()
        )));
    }
    let main_ts = "import \"./style.css\";\n\ndocument.querySelector(\"#app\")!.innerHTML = `<h1>hello ferrite</h1>`;\n";
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
    println!("created {} (template: {})", dir.display(), args.template);
    println!("  cd {}", dir.display());
    println!("  ferrite dev");
    Ok(())
}

fn validate_template(template: &str) -> ferrite::Result<()> {
    match template {
        "vanilla" => Ok(()),
        "ssr" => Err(ferrite::FerriteError::Other(
            "the legacy ssr template has no validated renderer/hydration profile; use vanilla for a client application".into(),
        )),
        _ => Err(ferrite::FerriteError::Other(format!(
            "unknown template `{template}`; available template: vanilla"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fail_closed() {
        assert!(validate_template("vanilla").is_ok());
        for template in ["vue", "svelte", "ssr", "vanila", ""] {
            assert!(validate_template(template).is_err(), "{template}");
        }
    }
}
