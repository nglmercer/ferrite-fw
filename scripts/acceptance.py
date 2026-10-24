#!/usr/bin/env python3
"""Execute explicit Ferrite acceptance gates and retain commit-specific evidence.

No browser/package installation is performed. Required fixtures fail rather than
silently skipping; `--list` only prints the plan and never records a pass.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def exact(package, target, name, features=None):
    command = ["cargo", "test", "--locked", "-p", package, "--test", target]
    if features:
        command += ["--features", features]
    return command + [name, "--", "--ignored", "--exact", "--nocapture"]


def plan(profile):
    commands = []
    if profile in ("core", "all"):
        commands.append(["cargo", "test", "--workspace", "--exclude", "ferrite-e2e", "--locked"])
    if profile in ("runtime", "all"):
        commands.append(["cargo", "test", "--locked", "-p", "ferrite-runtime", "-p", "ferrite-ssr", "-p", "ferrite-config", "--features", "ferrite-runtime/napi-vm,ferrite-ssr/napi-vm", "--lib"])
    if profile in ("runtime", "all"):
        commands.append(["cargo", "test", "--locked", "-p", "ferrite-runtime", "--features", "napi-vm", "--lib", "napi_vm::syntax_cache::tests::parser_preflight_cold_and_cached_measurement", "--", "--ignored", "--exact", "--nocapture"])
    if profile in ("frameworks", "all"):
        commands.append(["cargo", "build", "--locked", "-p", "ferrite-cli", "--features", "napi-vm"])
        commands.append(exact("ferrite-frameworks", "compiler_host", "actual_project_matched_compilers_client_server_and_runes"))
        for name in ("official_svelte_server_renderer_executes_emitted_graph", "official_vue_server_renderer_executes_emitted_graph", "official_components_resources_maps_cache_and_library_parity", "generated_vue_ssr_sources_install_build_and_render", "generated_svelte_ssr_sources_install_build_and_render", "generated_react_ssr_sources_install_build_and_render", "official_vue_ssr_hydrates_in_chromium", "official_vue_ssr_hydrates_in_firefox", "official_svelte_ssr_hydrates_in_chromium", "official_svelte_ssr_hydrates_in_firefox", "generated_vue_ssr_dev_graph_updates_and_recovers", "generated_vue_dev_ssr_chromium", "generated_vue_dev_ssr_firefox"):
            commands.append(exact("ferrite-test", "framework_compilers", name, "napi-vm"))
        commands.append(["cargo", "test", "--locked", "-p", "ferrite-test", "--features", "napi-vm", "--test", "prerender"])
        # Both engines and all existing generated-client/Refresh/foreign-hook
        # acceptance cases execute explicitly, rather than relying on unit tests.
        for engine in ("chromium", "firefox"):
            for suffix in ("generated_profiles", "generated_components", "dependency_acceptance", "transitive_self_acceptance", "initial_load_recovery", "configured_foreign_hooks", "react_refresh_dom", "react_refresh_wrapped", "react_refresh_imported_hook", "react_refresh_hook_barrel", "react_refresh_default_named", "react_refresh_default_anonymous_memo", "react_refresh_mixed_exports", "generated_react_profiles"):
                commands.append(exact("ferrite-test", "generated_apps", f"{engine}_{suffix}", "napi-vm"))
        for name in ("cli_vue_ssr_generation_and_clean_frozen_reinstall", "cli_vue_ssr_create_dev_build_preview_cycle", "cli_vue_ssr_base_path_cycle"):
            commands.append(exact("ferrite-test", "generated_apps", name, "napi-vm"))
        commands.append(["cargo", "test", "--locked", "-p", "ferrite-plugin", "--", "--ignored"])
    if profile in ("standalone", "all"):
        for name in ("standalone_build_compiles_and_serves", "standalone_ssr_build_compiles_and_renders_without_node"):
            commands.append(exact("ferrite-test", "standalone", name, "napi-vm"))
    if profile in ("e2e", "all"):
        commands.append(["cargo", "test", "--locked", "-p", "ferrite-e2e", "--", "--test-threads=1"])
    return commands


def environment_evidence(env):
    tools = {}
    candidates = {"rustc": shutil.which("rustc"), "cargo": shutil.which("cargo"), "node": shutil.which("node"), "chromium": env.get("FERRITE_CHROMIUM_PATH"), "firefox": env.get("FERRITE_FIREFOX_PATH")}
    for name, executable in candidates.items():
        if not executable:
            tools[name] = {"status": "unavailable"}
            continue
        try:
            result = subprocess.run([executable, "--version"], env=env, capture_output=True, text=True, timeout=5)
            tools[name] = {"path": str(Path(executable).resolve()), "status": "identified" if result.returncode == 0 else "failed", "version": result.stdout.strip(), "stderr": result.stderr.strip()}
        except (OSError, subprocess.TimeoutExpired) as error:
            tools[name] = {"path": str(executable), "status": "failed", "error": str(error)}
    return {"os": platform.system(), "architecture": platform.machine(), "tools": tools}


def run(args):
    commands = plan(args.profile)
    if args.list:
        print(json.dumps(commands, indent=2))
        return 0
    env = dict(os.environ)
    if args.profile in ("core", "frameworks", "e2e", "all"):
        for variable, names in (("FERRITE_CHROMIUM_PATH", ("chromium", "google-chrome", "chrome-headless-shell")), ("FERRITE_FIREFOX_PATH", ("firefox",))):
            binary = env.get(variable) or next((path for name in names if (path := shutil.which(name))), None)
            if not binary or not Path(binary).is_file():
                raise RuntimeError(f"required real browser missing: set {variable}; no gate was run")
            env[variable] = str(Path(binary).resolve())
        env["FERRITE_E2E_REQUIRE_BOTH_BROWSERS"] = "1"
    if args.profile in ("frameworks", "all"):
        if not shutil.which("node"):
            raise RuntimeError("framework compiler/plugin acceptance requires explicitly installed Node")
        # Respect Cargo's target directory when locating the freshly built CLI.
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"], cwd=ROOT, env=env))
        env["FERRITE_CLI_PATH"] = str(Path(metadata["target_directory"]) / "debug" / ("ferrite.exe" if os.name == "nt" else "ferrite"))
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report = {"schema": 1, "profile": args.profile, "revision": revision, "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)), "platform": platform.platform(), "started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(), "status": "running", "gates": []}
    report["environment"] = environment_evidence(env)
    output = args.report.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    try:
        for index, command in enumerate(commands):
            print("+ " + " ".join(command), flush=True)
            started = time.monotonic()
            log = output.with_name(f"{output.stem}-{index:02}.log")
            with log.open("w") as stream:
                process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=os.name != "nt")
                try:
                    process.wait(timeout=args.timeout)
                except BaseException:
                    if os.name == "nt":
                        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    else:
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    process.wait()
                    report["gates"].append({"command": command, "status": "interrupted", "seconds": round(time.monotonic() - started, 3), "log": log.name})
                    raise
            text = log.read_text(errors="replace")
            print(text, end="", flush=True)
            status = "passed" if process.returncode == 0 else "failed"
            # Exact filtered Cargo commands can succeed after a test is renamed.
            # A release gate must prove that the requested fixture actually ran.
            if "--exact" in command and not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", text):
                status = "failed"
            report["gates"].append({"command": command, "status": status, "exit_code": process.returncode, "seconds": round(time.monotonic() - started, 3), "log": log.name})
            if status != "passed":
                raise RuntimeError(f"acceptance failed; see {log}")
        report["status"] = "passed"
        return 0
    except BaseException:
        report["status"] = "failed"
        raise
    finally:
        output.write_text(json.dumps(report, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("core", "runtime", "frameworks", "standalone", "e2e", "all"), default="runtime")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--report", type=Path, default=ROOT / ".ferrite/acceptance/report.json")
    parser.add_argument("--timeout", type=int, default=1800, help="maximum seconds per gate")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    try:
        return run(args)
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        print(f"acceptance: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
