//! The host adapters cover the registered tool setup, native check handoff and release program.
//! The shell reader checks the observed commands, platform cases and selected control dependencies.
//! The release YAML reader supplies typed fields; Actions executes job dependencies.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use chrono_diagnostics::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub enum Manager { Existing, Platform, Unavailable }
pub fn manager(available: bool, hosted: bool) -> (action: Manager)
    ensures available ==> action == Manager::Existing,
        !available && hosted ==> action == Manager::Platform,
        !available && !hosted ==> action == Manager::Unavailable,
{
    if available { Manager::Existing } else if hosted { Manager::Platform } else { Manager::Unavailable }
}

/// These are the consecutive commands rendered into source.steps[1].run.
/// The shell is required to start with errexit; a failed command ends the job.
#[derive(Clone, Copy)]
pub enum Stage { Prepare, Download, VerifyAndChmod, CheckTag }
pub fn source_steps() -> (steps: Vec<Stage>)
    ensures steps@ == seq![Stage::Prepare, Stage::Download, Stage::VerifyAndChmod, Stage::CheckTag],
{
    let mut steps = Vec::new();
    steps.push(Stage::Prepare);
    steps.push(Stage::Download);
    steps.push(Stage::VerifyAndChmod);
    steps.push(Stage::CheckTag);
    steps
}

/// GitHub applies its default success condition to a required predecessor.
pub fn requires_success(needs_id: &String, predecessor: &String, explicit_if: bool, allow_failure: bool) -> (accepted: bool)
    ensures accepted == (needs_id@ == predecessor@ && !explicit_if && !allow_failure),
{ *needs_id == *predecessor && !explicit_if && !allow_failure }

/// A mise setup installs tools first and refreshes the rules only when selected.
pub fn setup_sequence(commands: &[String], instructions: bool) -> (accepted: bool)
    ensures accepted == (commands.len() == (if instructions { 2usize } else { 1usize })
        && commands@[0]@ == "sh .chrono-harness/install-tools.sh"@
        && (!instructions || commands@[1]@ == "mise exec -- chrono-instructions sync-rules --host-root ."@)),
{
    if commands.len() != (if instructions { 2usize } else { 1usize }) { return false; }
    commands[0] == "sh .chrono-harness/install-tools.sh".to_string() && (!instructions
        || commands[1] == "mise exec -- chrono-instructions sync-rules --host-root .".to_string())
}

/// The public native route transfers the process to the host's installed scratch owner.
pub fn native_owner_command(argv: &[String]) -> (accepted: bool)
    ensures accepted == (argv.len() == 7
        && argv@[0]@ == "exec"@ && argv@[1]@ == "chrono-host-bootstrap"@
        && argv@[2]@ == "native-scratch"@ && argv@[3]@ == "run"@
        && argv@[4]@ == "/bin/bash"@ && argv@[5]@ == "./check.sh"@
        && argv@[6]@ == "--native-in-directory"@),
{
    argv.len() == 7 && argv[0] == "exec".to_string() && argv[1] == "chrono-host-bootstrap".to_string()
        && argv[2] == "native-scratch".to_string() && argv[3] == "run".to_string()
        && argv[4] == "/bin/bash".to_string() && argv[5] == "./check.sh".to_string()
        && argv[6] == "--native-in-directory".to_string()
}

pub fn check_entrypoint() -> (s: &'static str) { include_str!("../../check.sh") }

pub fn mise_manifest() -> (s: &'static str) {
    include_str!("../../.config/mise/conf.d/chrono-harness.toml")
}

/// The template renders these same rows into both mise platform tables.
pub fn installed_asset(linux: bool) -> (fields: (&'static str, usize))
    ensures linux ==> fields.0@ == "linux-x86_64"@ && fields.1 == 1,
        !linux ==> fields.0@ == "macos-aarch64"@ && fields.1 == 2,
{
    if linux {
        ("linux-x86_64", 1)
    } else {
        ("macos-aarch64", 2)
    }
}

/// mise reports file/key presence and applies the selected Rust-regex expression.
#[derive(Clone, Copy)]
pub enum InstalledValue { Pin, Use, Reject }

pub fn installed_value(present: bool, matches: bool) -> (action: InstalledValue)
    ensures !present ==> action == InstalledValue::Pin,
        present && matches ==> action == InstalledValue::Use,
        present && !matches ==> action == InstalledValue::Reject,
{
    if !present { InstalledValue::Pin } else if matches { InstalledValue::Use }
    else { InstalledValue::Reject }
}

pub fn installed_pattern(is_version: bool) -> (pattern: &'static str)
    ensures is_version ==> pattern@ == "^[0-9]+[.][0-9]+[.][0-9]+(-[A-Za-z0-9-]+([.][A-Za-z0-9-]+)*)?$"@,
        !is_version ==> pattern@ == "^sha256:[a-f0-9]{64}$"@,
{
    if is_version { "^[0-9]+[.][0-9]+[.][0-9]+(-[A-Za-z0-9-]+([.][A-Za-z0-9-]+)*)?$" } else { "^sha256:[a-f0-9]{64}$" }
}

pub fn selected_mise_manifest() -> (s: &'static str) {
    include_str!("../../.config/mise/conf.d/selected-chrono-harness.toml")
}

pub fn tool_installer() -> (s: &'static str) { include_str!("../install-tools.sh") }

pub fn installer() -> (s: &'static str) { include_str!("install.sh") }
pub fn workflow() -> (s: &'static str) { include_str!("../../.github/workflows/release.yml") }

/// Both deployed files are inputs to this proof compilation and its native test.
pub fn deployed_roadmap() -> (&'static str, &'static str) {
    (include_str!("../../.github/roadmap.cjs"), include_str!("../../.github/workflows/issue-roadmap.yml"))
}

pub fn deployed_roadmap_lock() -> &'static str {
    include_str!("../../.github/workflows/actions.lock")
}

pub fn roadmap_repository(checkout: bool, owner_id: u64, repo_id: u64) -> (accepted: bool)
    ensures accepted == (owner_id == 44036562 && ((checkout && repo_id == 197814629)
        || (!checkout && repo_id == 205262760))),
{
    owner_id == 44036562 && repo_id == (if checkout { 197814629 } else { 205262760 })
}
/// The native caller hashes the actual UTF-8 bytes embedded above.
pub fn roadmap_matches(script: &String, workflow: &String) -> (accepted: bool)
    ensures accepted == (script@ == "5824ea576bd25a792c76e976dc2dcec1c7eb684f6a3a9de9919220a546b61cf9"@ && workflow@ == "ece23ae4408cce3db5b40eea2d1f6e7fb3bb7312e24cbda8beec0ade2ff49452"@),
{
    *script == "5824ea576bd25a792c76e976dc2dcec1c7eb684f6a3a9de9919220a546b61cf9".to_string() && *workflow == "ece23ae4408cce3db5b40eea2d1f6e7fb3bb7312e24cbda8beec0ade2ff49452".to_string()
}
}

#[cfg(not(verus_keep_ghost))]
mod native {
    use super::*;
    use serde_yaml::Value;

    fn when_selected(condition: &str, installed: &str, pinned: &str) -> String {
        format!("{{% if {condition} %}}{installed}{{% else %}}{pinned}{{% endif %}}")
    }

    // Reconstruct the actions of the host's actual mise expressions for every
    // outcome of its built-in file, TOML lookup and regex operations.
    fn installed_field_expression(is_version: bool, command: &str, index: usize) -> String {
        let (value, lookup, field) = if is_version {
            ("version", format!(r#"data["{command}"][0]"#), "version")
        } else {
            (
                "hash",
                format!(r#"read_toml(path=vars.chrono_selection)["{command}"][{index}]"#),
                "checksum",
            )
        };
        let action = |present, matches| match installed_value(present, matches) {
            InstalledValue::Pin if is_version => String::new(),
            InstalledValue::Pin => r#"{{ PLATFORM["checksum"] }}"#.to_owned(),
            InstalledValue::Use => format!("{{{{ {value} }}}}"),
            InstalledValue::Reject => {
                format!(r#"{{{{ throw(message="Invalid installed release {field}") }}}}"#)
            }
        };
        let assign = format!("{{% set {value} = {lookup} %}}");
        let pattern = installed_pattern(is_version);
        let check = format!(r#"{{% if {value} is matching(pat="{pattern}") %}}"#);
        let yes = action(true, true);
        let no = action(true, false);
        let fallback = action(false, false);
        let end = "{% endif %}";
        let otherwise = "{% else %}";
        let inner = format!("{assign}{check}{yes}{otherwise}{no}{end}");
        if is_version {
            let file = "{% if vars.chrono_selection is exists %}{% set data = read_toml(path=vars.chrono_selection) %}{% endif %}";
            let has = format!(r#"{{% if data is defined and "{command}" in data %}}"#);
            format!("{file}{has}{inner}{end}{fallback}")
        } else {
            when_selected("SELECTED", &inner, &fallback)
        }
    }

    #[test]
    fn installed_assets_use_the_verified_platforms() {
        let document: toml::Value = toml::from_str(selected_mise_manifest()).unwrap();
        assert_eq!(
            document["vars"]["chrono_selection"].as_str(),
            Some("{{ config_root }}/.chrono-harness/state/tool-selection/versions.toml")
        );
        assert_eq!(
            document["vars"]["chrono_pins"].as_str(),
            Some("{{ config_root }}/.config/mise/conf.d/chrono-harness.toml")
        );
        let entries = document["tools"].as_table().unwrap();
        assert_eq!(entries.len(), 21);
        for (key, tool) in entries {
            let command = key.strip_prefix("http:").unwrap();
            let repository = match command {
                "chrono-cache" => "chrono-harness/chrono-harness-cache",
                "chrono-ci" => "chrono-harness/chrono-harness-ci",
                "chrono-diagnostics" => "chrono-harness/chrono-harness-diagnostics",
                "chrono-distribution" => "chrono-harness/chrono-harness-distribution",
                "chrono-harness" => "chrono-harness/chrono-harness-runner",
                "chrono-host-bootstrap" => "chrono-harness/chrono-harness-host-bootstrap",
                "chrono-host-release" => "chrono-harness/chrono-harness-host-release",
                "chrono-inputs" => "chrono-harness/chrono-harness-inputs",
                "chrono-instructions" => "chrono-harness/chrono-harness-instructions",
                "chrono-judge-ci" => "chrono-harness/chrono-harness-judge-ci",
                "chrono-judge-cost" => "chrono-harness/chrono-harness-judge-cost",
                "chrono-judge-filemap" => "chrono-harness/chrono-harness-judge-filemap",
                "chrono-judge-mixed" => "chrono-harness/chrono-harness-judge-mixed",
                "chrono-judge-projects" => "chrono-harness/chrono-harness-judge-projects",
                "chrono-judge-registration" => "chrono-harness/chrono-harness-judge-registration",
                "chrono-judge-routes" => "chrono-harness/chrono-harness-judge-routes",
                "chrono-judge-workflow" => "chrono-harness/chrono-harness-judge-workflow",
                "chrono-pr" => "chrono-harness/chrono-harness-pr",
                "chrono-test-git" => "chrono-harness/chrono-harness-test-git",
                "chrono-test-git-profile" => "chrono-harness/chrono-harness-test-git",
                "chrono-worktree" => "chrono-harness/chrono-harness-worktree",
                _ => panic!("unexpected selected tool {command}"),
            };
            let variable = format!("selected_{}", command.replace('-', "_"));
            assert_eq!(
                document["vars"][&variable].as_str(),
                Some(installed_field_expression(true, command, 0).as_str())
            );
            let selected = format!("vars.{variable}");
            let value = format!("{{{{ {selected} }}}}");
            let pin_tool =
                format!(r#"read_toml(path=vars.chrono_pins)["tools"]["http:{command}"]"#);
            let pinned_version = format!(r#"{{{{ {pin_tool}["version"] }}}}"#);
            let version = when_selected(&selected, &value, &pinned_version);
            assert_eq!(tool["version"].as_str(), Some(version.as_str()));
            let platforms = tool["platforms"].as_table().unwrap();
            assert_eq!(platforms.len(), 2);
            for (name, linux) in [("linux-x64", true), ("macos-arm64", false)] {
                let (suffix, index) = installed_asset(linux);
                let pin = format!(r#"{pin_tool}["platforms"]["{name}"]"#);
                let asset = format!(
                    "https://github.com/{repository}/releases/download/v{value}/{command}-{suffix}"
                );
                let pinned_url = format!(r#"{{{{ {pin}["url"] }}}}"#);
                let url = when_selected(&selected, &asset, &pinned_url);
                let checksum = installed_field_expression(false, command, index)
                    .replace("SELECTED", &selected)
                    .replace("PLATFORM", &pin);
                assert_eq!(platforms[name]["url"].as_str(), Some(url.as_str()));
                assert_eq!(
                    platforms[name]["checksum"].as_str(),
                    Some(checksum.as_str())
                );
            }
        }
    }

    fn roadmap_digest(source: &str) -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn locked_roadmap_reference<'a>(
        lock: &Value,
        workflow: &'a [Value],
        repo: &str,
        checkout: bool,
    ) -> Option<&'a str> {
        let mut matched = None;
        for item in workflow {
            let name = item.as_str()?;
            let (source, reference) = name.split_once('@')?;
            if !source.eq_ignore_ascii_case(repo) {
                continue;
            }
            if source != repo || matched.is_some() {
                return None;
            }
            let entry = lock["dependencies"].get(name)?;
            let sha = entry["commit"].as_str()?.strip_prefix("sha1-")?;
            if reference.is_empty()
                || !reference
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'))
                || entry["ref"].as_str() != Some(reference)
                || entry
                    .get("hostname")
                    .is_some_and(|v| v.as_str() != Some("github.com"))
                || sha.len() != 40
                || !sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || sha.bytes().all(|b| b == b'0')
                || !roadmap_repository(
                    checkout,
                    entry["owner_id"].as_u64()?,
                    entry["repo_id"].as_u64()?,
                )
            {
                return None;
            }
            matched = Some(name);
        }
        matched
    }

    fn roadmap_structure(workflow: &str, lock: &str) -> Option<String> {
        let lock: Value = serde_yaml::from_str(lock).ok()?;
        if lock["version"].as_str()? != "v0.0.3" {
            return None;
        }
        let mapping = lock["workflows"][".github/workflows/issue-roadmap.yml"].as_sequence()?;
        let checkout = locked_roadmap_reference(&lock, mapping, "actions/checkout", true)?;
        let script = locked_roadmap_reference(&lock, mapping, "actions/github-script", false)?;
        let mut projected = String::with_capacity(workflow.len());
        for line in workflow.split_inclusive('\n') {
            if let Some(value) = line.strip_prefix("      - uses: ") {
                let repository = match value.strip_suffix('\n')? {
                    v if v == checkout => "actions/checkout",
                    v if v == script => "actions/github-script",
                    _ => return None,
                };
                projected.push_str("      - uses: ");
                projected.push_str(repository);
                projected.push_str("@LOCKED\n");
            } else {
                projected.push_str(line);
            }
        }
        Some(projected)
    }

    #[test]
    fn locked_roadmap_rejects_changes_to_its_deployment_contract() {
        let (script, workflow) = deployed_roadmap();
        let lock = deployed_roadmap_lock();
        let script_hash = roadmap_digest(script);
        let accepted = |candidate: &str, lock: &str| {
            roadmap_structure(candidate, lock)
                .is_some_and(|structure| roadmap_matches(&script_hash, &roadmap_digest(&structure)))
        };
        for (expected, changed) in [
            ("actions/checkout@", "other/checkout@"),
            ("contents: read", "contents: write"),
            ("    steps:\n", "    steps:\n      - run: echo extra\n"),
            (".releaseIssue(", ".different("),
        ] {
            let edit = workflow.replacen(expected, changed, 1);
            assert_ne!(edit, workflow);
            assert!(!accepted(&edit, lock));
        }
        for expected in [
            ".github/workflows/issue-roadmap.yml",
            "197814629",
            "205262760",
        ] {
            let edit = lock.replace(expected, "absent");
            assert_ne!(edit, lock);
            assert!(!accepted(workflow, &edit));
        }
        assert!(!accepted(workflow, ""));
    }

    #[test]
    fn deployed_roadmap_matches_the_selected_template() {
        let (script, workflow) = deployed_roadmap();
        let script_hash = roadmap_digest(script);
        let structure = roadmap_structure(workflow, deployed_roadmap_lock())
            .expect("the roadmap action references must belong to their GitHub workflow lock");
        let workflow_hash = roadmap_digest(&structure);
        assert!(
            roadmap_matches(&script_hash, &workflow_hash),
            "roadmap.cjs or issue-roadmap.yml differs from the selected template"
        );
        assert!(!roadmap_matches(&workflow_hash, &workflow_hash));
        assert!(!roadmap_matches(&script_hash, &script_hash));
    }

    fn text<'a>(node: tree_sitter::Node<'_>, source: &'a str) -> &'a str {
        node.utf8_text(source.as_bytes()).unwrap()
    }

    #[derive(Default)]
    struct InstallEvidence {
        calls: Vec<(String, String)>,
        decisions: Vec<(String, String)>,
        guards: Vec<String>,
        branches: Vec<(String, String)>,
        assignments: Vec<String>,
        redirects: Vec<String>,
        logic: Vec<String>,
        scopes: Vec<String>,
    }

    fn shell_tree(source: &str) -> Option<tree_sitter::Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .unwrap();
        parser
            .parse(source, None)
            .filter(|tree| !tree.root_node().has_error())
    }

    fn observe(node: tree_sitter::Node<'_>, src: &str, arm: &str, e: &mut InstallEvidence) {
        if matches!(
            node.kind(),
            "if_statement" | "else_clause" | "for_statement" | "do_group"
        ) {
            let mut cursor = node.walk();
            e.scopes.push(
                node.named_children(&mut cursor)
                    .filter(|child| child.kind() != "comment")
                    .map(|child| child.kind())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        let mut context = arm;
        let child = |i| text(node.named_child(i).unwrap(), src);
        match node.kind() {
            "case_item" => {
                context = child(0);
                if context.starts_with("manager-")
                    || context.starts_with("platform-")
                    || context == "*"
                {
                    let mut cursor = node.walk();
                    let values: Vec<_> = node
                        .named_children(&mut cursor)
                        .filter(|c| c.kind() == "variable_assignment")
                        .map(|c| text(c, src))
                        .collect();
                    e.decisions.push((context.to_owned(), values.join(" ")));
                }
            }
            "command" => e.calls.push((arm.to_owned(), text(node, src).to_owned())),
            "test_command" => e.guards.push(text(node, src).to_owned()),
            "variable_assignment"
                if !arm.starts_with("manager-") && !arm.starts_with("platform-") && arm != "*" =>
            {
                e.assignments.push(text(node, src).to_owned())
            }
            "file_redirect" => e.redirects.push(text(node, src).to_owned()),
            "if_statement" | "case_statement" | "for_statement" | "function_definition" => e
                .branches
                .push((node.kind().to_owned(), child(0).to_owned())),
            "list" | "pipeline" | "negated_command" | "subshell" | "declaration_command" => {
                e.logic.push(text(node, src).to_owned())
            }
            // Backgrounding or falling through to another case arm changes dispatch.
            "&" | ";&" | ";;&" | "while_statement" | "elif_clause" => {
                e.logic.push(node.kind().to_owned())
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            observe(child, src, context, e);
        }
    }

    #[test]
    fn public_native_check_starts_the_scratch_owner() {
        let source = check_entrypoint();
        let tree = shell_tree(source).unwrap();
        let root = tree.root_node();
        let mut cursor = root.walk();
        let route = root
            .named_children(&mut cursor)
            .find(|node| {
                node.kind() == "if_statement"
                    && node
                        .child_by_field_name("condition")
                        .is_some_and(|condition| {
                            &source[condition.byte_range()] == "[[ $# == 1 && $1 == --native ]]"
                        })
            })
            .expect("the public --native branch");
        let mut cursor = route.walk();
        let commands: Vec<_> = route
            .named_children(&mut cursor)
            .filter(|node| node.kind() == "command")
            .collect();
        assert_eq!(commands.len(), 1);
        let mut cursor = commands[0].walk();
        let mut argv: Vec<_> = commands[0]
            .named_children(&mut cursor)
            .map(|node| source[node.byte_range()].to_owned())
            .collect();
        assert!(native_owner_command(&argv));
        argv.remove(0);
        assert!(!native_owner_command(&argv));
    }

    fn setup_matches(source: &str, instructions: bool) -> bool {
        let Ok(doc) = source.parse::<toml::Table>() else {
            return false;
        };
        let instruction_tool = doc
            .get("tools")
            .and_then(toml::Value::as_table)
            .is_some_and(|tools| tools.contains_key("http:chrono-instructions"));
        let Some(setup) = doc
            .get("tasks")
            .and_then(|tasks| tasks.get("harness-setup"))
            .and_then(toml::Value::as_table)
        else {
            return false;
        };
        if instruction_tool != instructions
            || setup.keys().any(|key| key != "run" && key != "description")
        {
            return false;
        }
        let commands: Vec<String> = match setup.get("run") {
            Some(toml::Value::String(one)) => vec![one.clone()],
            Some(toml::Value::Array(steps)) => {
                let Some(all) = steps
                    .iter()
                    .map(|value| value.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                all
            }
            _ => return false,
        };
        setup_sequence(&commands, instructions)
    }

    #[test]
    fn every_installed_organization_tool_can_follow_the_selected_channel() {
        use std::collections::BTreeSet;

        let manifest = mise_manifest().parse::<toml::Table>().unwrap();
        let expected: BTreeSet<_> = manifest["tools"]
            .as_table()
            .unwrap()
            .keys()
            .filter_map(|name| name.strip_prefix("http:chrono-"))
            .collect();
        let (_, rows) = tool_installer().split_once("done <<'TOOLS'\n").unwrap();
        let (rows, _) = rows.split_once("\nTOOLS\n").unwrap();
        let mut selected = BTreeSet::new();
        for row in rows.lines() {
            let (command, _) = row.split_once(' ').unwrap();
            assert!(
                selected.insert(command.strip_prefix("chrono-").unwrap()),
                "{command}"
            );
        }
        assert_eq!(selected, expected);

        let projection = selected_mise_manifest().parse::<toml::Table>().unwrap();
        let projected: BTreeSet<_> = projection["tools"]
            .as_table()
            .unwrap()
            .iter()
            .map(|(key, tool)| {
                let command = tool["bin"].as_str().unwrap();
                assert_eq!(key.strip_prefix("http:"), Some(command));
                command.strip_prefix("chrono-").unwrap()
            })
            .collect();
        assert_eq!(projected, expected);
    }

    #[test]
    fn mise_installs_the_selected_tools_before_refreshing_instructions() {
        let original = mise_manifest();
        let selected = true;
        assert!(setup_matches(original, selected));
        assert!(!setup_matches(
            &original.replace(
                "sh .chrono-harness/install-tools.sh",
                "sh .chrono-harness/install-tools.sh --dry-run"
            ),
            selected
        ));
        assert!(!setup_matches(
            &original.replace(
                "[tasks.harness-setup]",
                "[tasks.harness-setup]\ndepends = ['other']"
            ),
            selected
        ));
        let install_only =
            "[tools]\n[tasks.harness-setup]\nrun = 'sh .chrono-harness/install-tools.sh'";
        assert!(setup_matches(install_only, false));
        assert!(!setup_matches(install_only, true));
        let reverse = "[tools.'http:chrono-instructions']\n[tasks.harness-setup]\nrun=['mise exec -- chrono-instructions sync-rules --host-root .','sh .chrono-harness/install-tools.sh']";
        assert!(!setup_matches(reverse, true));
        let extra = "[tools]\n[tasks.harness-setup]\nrun = ['sh .chrono-harness/install-tools.sh', 'another-command']";
        assert!(!setup_matches(extra, false));
    }

    fn bootstrap_matches(src: &str) -> bool {
        let Some(tree) = shell_tree(src) else {
            return false;
        };
        let mut e = InstallEvidence::default();
        observe(tree.root_node(), src, "main", &mut e);
        let mut expected = Vec::new();
        for available in [false, true] {
            for hosted in [false, true] {
                let action = match manager(available, hosted) {
                    Manager::Existing => "existing",
                    Manager::Platform => "platform",
                    Manager::Unavailable => "unavailable",
                };
                expected.push((
                    format!("manager-{}{}", u8::from(available), u8::from(hosted)),
                    format!("choice={action}"),
                ));
            }
        }
        expected.extend([
            ("platform-Darwin/arm64".into(),"choice=macos-arm64 digest=87b2bd5a1b88a612ee8856df23499404b115870b5a8a8e008763c7cd51c8d412".into()),
            ("platform-Linux/x86_64".into(),"choice=linux-x64 digest=9c12c24f5eac6b6357e5265e2629166cc3005459037f5d747680a99e375ff2ab".into()),
            ("*".into(),"choice=unsupported".into()),
        ]);
        let required: &[(&str, &str)] = &[
            ("main", "set -eu"),
            ("main", "command -v mise"),
            (
                "main",
                r#"ci_install_choice "manager-$manager_available$hosted""#,
            ),
            (
                "platform",
                r#"ci_install_choice "platform-$(uname -s)/$(uname -m)""#,
            ),
            ("platform", "uname -s"),
            ("platform", "uname -m"),
            ("unsupported", "echo 'Unsupported runner platform'"),
            ("unsupported", "exit 1"),
            (
                "platform",
                r#"mktemp -d "$RUNNER_TEMP/diagnostics-mise.XXXXXXXX""#,
            ),
            ("platform", r#"trap 'rm -rf -- "$stage"' EXIT"#),
            (
                "platform",
                r#"curl --fail --silent --show-error --location "https://github.com/jdx/mise/releases/download/v2026.11.12/mise-v2026.11.12-$asset" --output "$manager""#,
            ),
            ("platform", r#"cd "$stage""#),
            ("platform", r#"echo "$digest  mise""#),
            ("platform", "shasum -a 256 -c -"),
            ("platform", r#"chmod 700 "$manager""#),
            (
                "unavailable",
                "echo 'Install mise and run mise trust in the checkout'",
            ),
            ("unavailable", "exit 1"),
            (
                "main",
                "set -- chrono-harness chrono-worktree chrono-diagnostics chrono-ci chrono-judge-ci chrono-judge-routes chrono-judge-registration chrono-judge-projects chrono-judge-filemap chrono-host-bootstrap verus",
            ),
            ("main", "mkdir -p .chrono-harness/bin"),
            ("main", r#""$manager" which "$tool""#),
            ("main", r#""$manager" install "http:$tool""#),
            ("main", r#""$manager" which "$tool""#),
            (
                "main",
                r#"ln -sfn "$executable" ".chrono-harness/bin/$tool""#,
            ),
            ("main", r#"dirname "$executable""#),
        ];
        let calls: Vec<_> = e
            .calls
            .iter()
            .map(|(arm, cmd)| (arm.as_str(), cmd.as_str()))
            .collect();
        let hosted_guard = r#"[ "${GITHUB_ACTIONS:-}" = true ]"#;
        let path_guard = r#"[ -n "${GITHUB_PATH:-}" ]"#;
        let lookup = r#"executable=$("$manager" which "$tool" 2>/dev/null)"#;
        let checksum = r#"(cd "$stage" && echo "$digest  mise" | shasum -a 256 -c -)"#;
        let branches: Vec<_> = e
            .branches
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let scopes = [
            "redirected_statement variable_assignment else_clause",
            "variable_assignment",
            "test_command variable_assignment else_clause",
            "variable_assignment",
            "test_command declaration_command",
            "variable_name do_group",
            "if_statement command if_statement",
            "negated_command command variable_assignment",
            "test_command redirected_statement",
        ];
        src.starts_with("#!/bin/sh\n")
            && e.scopes == scopes
            && e.decisions == expected
            && calls == required
            && e.guards == [hosted_guard, hosted_guard, path_guard]
            && branches
                == [
                    ("function_definition", "ci_install_choice"),
                    ("case_statement", "$1"),
                    ("if_statement", "command -v mise >/dev/null 2>&1"),
                    ("if_statement", hosted_guard),
                    ("case_statement", "$choice"),
                    ("case_statement", "$choice"),
                    ("if_statement", hosted_guard),
                    ("for_statement", "tool"),
                    (
                        "if_statement",
                        r#"! executable=$("$manager" which "$tool" 2>/dev/null)"#,
                    ),
                    ("if_statement", path_guard),
                ]
            && e.assignments
                == [
                    "manager_available=1",
                    "manager_available=0",
                    "hosted=1",
                    "hosted=0",
                    "manager=mise",
                    "asset=$choice",
                    r#"stage=$(mktemp -d "$RUNNER_TEMP/diagnostics-mise.XXXXXXXX")"#,
                    "manager=$stage/mise",
                    r#"MISE_TRUSTED_CONFIG_PATHS="$PWD""#,
                    lookup,
                    r#"executable=$("$manager" which "$tool")"#,
                ]
            && e.redirects
                == [
                    ">/dev/null",
                    "2>&1",
                    ">&2",
                    ">&2",
                    "2>/dev/null",
                    r#">> "$GITHUB_PATH""#,
                ]
            && e.logic
                == [
                    checksum,
                    r#"cd "$stage" && echo "$digest  mise" | shasum -a 256 -c -"#,
                    r#"echo "$digest  mise" | shasum -a 256 -c -"#,
                    r#"export MISE_TRUSTED_CONFIG_PATHS="$PWD""#,
                    r#"! executable=$("$manager" which "$tool" 2>/dev/null)"#,
                ]
    }

    #[test]
    fn bootstrap_routes_and_arguments_bind_to_the_checked_script() {
        let src = installer();
        assert!(bootstrap_matches(src));
        assert!(bootstrap_matches(&src.replace(
            "mkdir -p .chrono-harness/bin",
            "# prepare local links\nmkdir -p .chrono-harness/bin"
        )));
        for (before, after) in [
            ("which \"$tool\"", "which broken"),
            ("shasum -a 256 -c -", "true"),
            ("&& echo", "|| echo"),
            ("choice=unavailable", "choice=existing"),
            ("exit 1", "exit 0"),
            ("case $1 in", "case $2 in"),
            ("for tool;", "for tool in one;"),
            ("hosted=1; else hosted=0", "hosted=0; else hosted=1"),
            ("manager=$stage/mise", "manager=mise"),
            (r#">> "$GITHUB_PATH""#, r#"> "$GITHUB_PATH""#),
        ] {
            assert!(src.contains(before));
            assert!(!bootstrap_matches(&src.replace(before, after)));
        }
    }

    fn needs(job: &Value, predecessor: &str) -> bool {
        job.get("needs").and_then(Value::as_str).is_some_and(|id| {
            requires_success(
                &id.to_owned(),
                &predecessor.to_owned(),
                job.get("if").is_some(),
                job.get("continue-on-error").is_some(),
            )
        })
    }

    fn tag_gate(doc: &Value) -> bool {
        // These are the effective provider inputs. A job-level override, a
        // different trigger, or a second publish job changes the graph.
        let Some(top) = doc.as_mapping() else {
            return false;
        };
        if top.len() != 4
            || doc["name"].as_str().is_none()
            || doc["permissions"].as_mapping().map(|v| v.len()) != Some(1)
            || doc["permissions"]["contents"].as_str() != Some("read")
            || doc["on"].as_mapping().map(|v| v.len()) != Some(1)
            || doc["on"]["push"].as_mapping().map(|v| v.len()) != Some(1)
            || doc["on"]["push"]["tags"].as_sequence().map(|v| v.len()) != Some(1)
            || doc["on"]["push"]["tags"][0].as_str() != Some("v*")
        {
            return false;
        }
        let jobs = &doc["jobs"];
        if jobs.as_mapping().map(|v| v.len()) != Some(3) {
            return false;
        }
        let source = &jobs["source"];
        let build = &jobs["build"];
        let publish = &jobs["publish"];
        let Some(source_map) = source.as_mapping() else {
            return false;
        };
        if source_map.len() != 3
            || source["runs-on"].as_str() != Some("ubuntu-24.04")
            || source["outputs"]["prerelease"].as_str()
                != Some("${{ steps.tag.outputs.prerelease }}")
        {
            return false;
        }
        let Some(steps) = source["steps"].as_sequence() else {
            return false;
        };
        if steps.len() != 2 {
            return false;
        }
        let checkout = &steps[0];
        let decision = &steps[1];
        if checkout.as_mapping().map(|x| x.len()) != Some(2)
            || checkout["uses"].as_str()
                != Some("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1")
            || checkout["with"].as_mapping().map(|x| x.len()) != Some(2)
            || checkout["with"]["fetch-depth"].as_i64() != Some(0)
            || checkout["with"]["persist-credentials"].as_bool() != Some(false)
            || decision.as_mapping().map(|x| x.len()) != Some(4)
            || decision["id"].as_str() != Some("tag")
            || decision["name"].as_str().is_none()
            || decision["shell"].as_str() != Some("bash")
        {
            return false;
        }
        let Some(shell) = decision["run"].as_str() else {
            return false;
        };
        let Some(requirements) = publish["needs"].as_sequence() else {
            return false;
        };
        let Some(publisher) = publish["steps"]
            .as_sequence()
            .and_then(|steps| steps.last())
        else {
            return false;
        };
        source_command_plan(shell)
            && needs(build, "source")
            && requirements.len() == 2
            && ["source", "build"].iter().all(|expected| {
                requirements.iter().any(|id| {
                    id.as_str().is_some_and(|id| {
                        requires_success(
                            &id.to_owned(),
                            &expected.to_string(),
                            publish.get("if").is_some(),
                            publish.get("continue-on-error").is_some(),
                        )
                    })
                })
            })
            && publisher["env"]["PRERELEASE"].as_str()
                == Some("${{ needs.source.outputs.prerelease }}")
            && publisher["run"].as_str().is_some_and(|shell| {
                shell
                    .split_ascii_whitespace()
                    .filter(|arg| arg.starts_with("--prerelease"))
                    .eq([r#"--prerelease="$PRERELEASE""#])
            })
    }

    // Read each top-level command from the real source step. Formatting and
    // comments between statements do not affect which dependent job can run.
    fn source_command_plan(shell: &str) -> bool {
        let Some(tree) = shell_tree(shell) else {
            return false;
        };
        let root = tree.root_node();
        let mut cursor = root.walk();
        let mut statements = root
            .named_children(&mut cursor)
            .filter(|node| node.kind() != "comment");
        let mut take = |kind: &str, args: &[&str]| {
            let Some(node) = statements.next() else {
                return false;
            };
            if node.kind() != kind {
                return false;
            }
            let mut cursor = node.walk();
            let actual: Vec<_> = node
                .named_children(&mut cursor)
                .map(|child| text(child, shell))
                .collect();
            actual == args
        };
        for stage in source_steps() {
            let valid = match stage {
                Stage::Prepare => {
                    take("command", &["set", "-euo", "pipefail"])
                        && take(
                            "variable_assignment",
                            &[
                                "directory",
                                r#"$(mktemp -d "$RUNNER_TEMP/tag-check.XXXXXXXX")"#,
                            ],
                        )
                        && take("command", &["trap", r#"'rm -rf -- "$directory"'"#, "EXIT"])
                }
                Stage::Download => take(
                    "command",
                    &[
                        "curl",
                        "--fail",
                        "--silent",
                        "--show-error",
                        "--location",
                        "'https://github.com/chrono-harness/chrono-harness-host-release/releases/download/v0.1.3/chrono-host-release-linux-x86_64'",
                        "--output",
                        r#""$directory/chrono-host-release""#,
                    ],
                ),
                Stage::VerifyAndChmod => {
                    take(
                        "subshell",
                        &[
                            r#"cd "$directory" && echo '96dd49fd2c6ad7bac8938ac351459e9b8751f7c917a7239e15994b26771a9a4e  chrono-host-release' | sha256sum --check --status"#,
                        ],
                    ) && take(
                        "command",
                        &["chmod", "700", r#""$directory/chrono-host-release""#],
                    )
                }
                Stage::CheckTag => {
                    take(
                        "variable_assignment",
                        &[
                            "prerelease",
                            r#"$("$directory/chrono-host-release" check-pushed-tag "$GITHUB_REF_NAME" --prerelease)"#,
                        ],
                    ) && take(
                        "redirected_statement",
                        &[
                            r#"printf 'prerelease=%s\n' "$prerelease""#,
                            r#">> "$GITHUB_OUTPUT""#,
                        ],
                    )
                }
            };
            if !valid {
                return false;
            }
        }
        !root.children(&mut root.walk()).any(|n| n.kind() == "&") && statements.next().is_none()
    }

    #[test]
    fn release_gate_binds_artifact_identity_tag_forwarding_and_dependencies() {
        let source = workflow();
        let doc: Value = serde_yaml::from_str(source).unwrap();
        assert!(tag_gate(&doc));
        for (original, replacement) in [
            ("--check --status", "--status"),
            (
                "check-pushed-tag \"$GITHUB_REF_NAME\"",
                "check-pushed-tag main",
            ),
            ("needs: source", "needs: source\n    if: always()"),
            ("needs: [source, build]", "needs: [source]"),
            ("needs: [source, build]", "needs: [build]"),
            (
                "steps.tag.outputs.prerelease",
                "steps.other.outputs.prerelease",
            ),
            (
                "needs.source.outputs.prerelease",
                "needs.build.outputs.prerelease",
            ),
            (r#"--prerelease="$PRERELEASE""#, "--prerelease=false"),
            ("  source:\n", "  source:\n    continue-on-error: true\n"),
            ("tags: ['v*']", "tags: ['*']"),
            ("echo '96dd49fd", "echo '06dd49fd"),
        ] {
            assert!(source.contains(original));
            let drift: Value =
                serde_yaml::from_str(&source.replace(original, replacement)).unwrap();
            assert!(!tag_gate(&drift));
        }
    }
}
