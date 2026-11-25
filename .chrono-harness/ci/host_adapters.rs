//! These two adapters use the registered bootstrap table and release program.
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
        && commands@[0]@ == "mise install"@
        && (!instructions || commands@[1]@ == "mise exec -- chrono-instructions sync-rules --host-root ."@)),
{
    if commands.len() != (if instructions { 2usize } else { 1usize }) { return false; }
    commands[0] == "mise install".to_string() && (!instructions
        || commands[1] == "mise exec -- chrono-instructions sync-rules --host-root .".to_string())
}

pub fn mise_manifest() -> (s: &'static str) {
    include_str!("../../.config/mise/conf.d/chrono-harness.toml")
}

pub fn installer() -> (s: &'static str) { include_str!("install.sh") }
pub fn workflow() -> (s: &'static str) { include_str!("../../.github/workflows/release.yml") }
}

#[cfg(not(verus_keep_ghost))]
mod native {
    use super::*;
    use serde_yaml::Value;

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

    fn setup_matches(source: &str, instructions: bool) -> bool {
        let Ok(doc) = source.parse::<toml::Table>() else {
            return false;
        };
        let Some(tools) = doc.get("tools").and_then(toml::Value::as_table) else {
            return false;
        };
        let Some(setup) = doc
            .get("tasks")
            .and_then(|tasks| tasks.get("harness-setup"))
            .and_then(toml::Value::as_table)
        else {
            return false;
        };
        if tools.contains_key("http:chrono-instructions") != instructions
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
    fn mise_installs_the_selected_tools_before_refreshing_instructions() {
        let original = mise_manifest();
        let selected = true;
        assert!(setup_matches(original, selected));
        assert!(!setup_matches(
            &original.replace("mise install", "mise install --dry-run"),
            selected
        ));
        assert!(!setup_matches(
            &original.replace(
                "[tasks.harness-setup]",
                "[tasks.harness-setup]\ndepends = ['other']"
            ),
            selected
        ));
        let install_only = "[tools]\n[tasks.harness-setup]\nrun = 'mise install'";
        assert!(setup_matches(install_only, false));
        assert!(!setup_matches(install_only, true));
        let reverse = "[tools.'http:chrono-instructions']\n[tasks.harness-setup]\nrun=['mise exec -- chrono-instructions sync-rules --host-root .','mise install']";
        assert!(!setup_matches(reverse, true));
        let extra = "[tools]\n[tasks.harness-setup]\nrun = ['mise install', 'another-command']";
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
        if source_map.len() != 2 || source["runs-on"].as_str() != Some("ubuntu-24.04") {
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
                != Some("actions/checkout@11d5960a326750d5838078e36cf38b85af677262")
            || checkout["with"].as_mapping().map(|x| x.len()) != Some(2)
            || checkout["with"]["fetch-depth"].as_i64() != Some(0)
            || checkout["with"]["persist-credentials"].as_bool() != Some(false)
            || decision.as_mapping().map(|x| x.len()) != Some(3)
            || decision["name"].as_str().is_none()
            || decision["shell"].as_str() != Some("bash")
        {
            return false;
        }
        let Some(shell) = decision["run"].as_str() else {
            return false;
        };
        source_command_plan(shell) && needs(build, "source") && needs(publish, "build")
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
                        "'https://github.com/chrono-harness/chrono-harness-host-release/releases/download/v0.1.0-alpha.21/chrono-host-release-linux-x86_64'",
                        "--output",
                        r#""$directory/chrono-host-release""#,
                    ],
                ),
                Stage::VerifyAndChmod => {
                    take(
                        "subshell",
                        &[
                            r#"cd "$directory" && echo '53f966841373df821abf14eb726907ed2c6192bf948e8743fa4952e0818fd5e3  chrono-host-release' | sha256sum --check --status"#,
                        ],
                    ) && take(
                        "command",
                        &["chmod", "700", r#""$directory/chrono-host-release""#],
                    )
                }
                Stage::CheckTag => take(
                    "command",
                    &[
                        r#""$directory/chrono-host-release""#,
                        "check-pushed-tag",
                        r#""$GITHUB_REF_NAME""#,
                    ],
                ),
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
            ("needs: build", "needs: source"),
            ("  source:\n", "  source:\n    continue-on-error: true\n"),
            ("tags: ['v*']", "tags: ['*']"),
            ("echo '53f96684", "echo '03f96684"),
        ] {
            assert!(source.contains(original));
            let drift: Value =
                serde_yaml::from_str(&source.replace(original, replacement)).unwrap();
            assert!(!tag_gate(&drift));
        }
    }
}
