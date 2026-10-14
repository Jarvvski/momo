use super::keys::{KNOWN_KEYS, KNOWN_PACKAGE_KEYS};
use super::*;

use std::path::Path;

#[test]
fn the_config_surface_is_frozen() {
    assert_eq!(
        KNOWN_KEYS,
        [
            "recording",
            "changelog",
            "changes_dir",
            "tag_prefix",
            "remote",
            "task_env",
            "rc_format",
            "command_timeout",
            "packages",
        ]
    );
    assert_eq!(
        KNOWN_PACKAGE_KEYS,
        ["identity", "slug", "manifest", "tag_prefix"]
    );
}

#[test]
fn defaults_match_the_spec() {
    let config = Config::default();
    assert_eq!(config.recording, Recording::Changelog);
    assert_eq!(config.changelog, PathBuf::from("CHANGELOG.md"));
    assert_eq!(config.changes_dir, PathBuf::from(".momo/changes"));
    assert_eq!(config.tag_prefix, "v");
    assert_eq!(config.remote, None);
    assert_eq!(config.task_env, None);
    assert_eq!(config.rc_format, None);
    assert_eq!(config.command_timeout, Duration::from_secs(60));
    assert!(config.packages.is_empty());
}

#[test]
fn the_rendered_defaults_parse_back_as_overrides() {
    // `init` writes `Config::default().to_string()`; the file it produces
    // must be one momo can read back.
    let text = Config::default().to_string();
    let overrides: ConfigOverrides = text.parse().expect("the rendered defaults are valid TOML");
    assert_eq!(
        Config::resolve(overrides, ConfigOverrides::default()),
        Config::default()
    );
}

#[test]
fn init_renders_exactly_the_frozen_default_keys() {
    // `momo init` writes the default configuration; its key set is part of the
    // frozen surface, so a new default key must be a deliberate edit here.
    let text = Config::default().to_string();
    let doc: toml_edit::DocumentMut = text.parse().expect("the rendered defaults are valid TOML");
    let mut keys: Vec<&str> = doc.as_table().iter().map(|(key, _)| key).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["changelog", "changes_dir", "recording", "tag_prefix"]
    );
}

#[test]
fn file_layer_overrides_defaults() {
    let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    assert_eq!(config.recording, Recording::Fragments);
    assert_eq!(config.changelog, PathBuf::from("CHANGELOG.md"));
}

#[test]
fn env_layer_overrides_file() {
    let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
    let env =
        ConfigOverrides::from_env([("MOMO_RECORDING", "changelog")]).expect("valid env value");
    let config = Config::resolve(file, env);
    assert_eq!(config.recording, Recording::Changelog);
}

#[test]
fn the_precedence_chain_is_monotonic() {
    let file: ConfigOverrides = "tag_prefix = \"file\"\n".parse().expect("valid TOML");
    let env = ConfigOverrides::from_env([("MOMO_TAG_PREFIX", "env")]).expect("valid env");
    assert_eq!(
        Config::resolve(file.clone(), ConfigOverrides::default()).tag_prefix,
        "file"
    );
    assert_eq!(Config::resolve(file, env).tag_prefix, "env");
}

#[test]
fn a_remote_is_parsed_from_the_file() {
    let file: ConfigOverrides = "remote = \"git@github.com:a/b.git\"\n"
        .parse()
        .expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    assert_eq!(
        config.remote.as_ref().map(Remote::base),
        Some("https://github.com/a/b")
    );
}

#[test]
fn a_remote_is_parsed_from_the_environment() {
    let env = ConfigOverrides::from_env([("MOMO_REMOTE", "https://github.com/a/b")])
        .expect("valid env value");
    let config = Config::resolve(ConfigOverrides::default(), env);
    assert_eq!(
        config.remote.as_ref().map(Remote::base),
        Some("https://github.com/a/b")
    );
}

#[test]
fn an_unparseable_remote_in_the_file_is_rejected() {
    let err = "remote = \"not a remote\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("an unparseable remote fails");
    let message = err.to_string();
    assert!(message.contains("remote"), "{message}");
    assert!(message.contains("not a remote"), "{message}");
}

#[test]
fn an_unparseable_remote_in_the_environment_is_rejected() {
    let err = ConfigOverrides::from_env([("MOMO_REMOTE", "not a remote")])
        .expect_err("an unparseable remote fails");
    let message = err.to_string();
    assert!(message.contains("MOMO_REMOTE"), "{message}");
    assert!(message.contains("not a remote"), "{message}");
}

#[test]
fn an_rc_format_without_exactly_one_marker_is_rejected() {
    for format in ["-rc", "-rc.{n}.{n}"] {
        let config = Config {
            rc_format: Some(format.to_owned()),
            ..Config::default()
        };
        let err = config
            .validate()
            .expect_err("a template without exactly one `{n}` is rejected");
        let message = err.to_string();
        assert!(message.contains("rc_format"), "{message}");
        assert!(message.contains(format), "{message}");
    }
}

#[test]
fn a_valid_rc_format_passes_validation() {
    let config = Config {
        rc_format: Some("-{n}.rc".to_owned()),
        ..Config::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn a_command_timeout_is_parsed_from_the_file() {
    let file: ConfigOverrides = "command_timeout = 5\n".parse().expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    assert_eq!(config.command_timeout, Duration::from_secs(5));
}

#[test]
fn a_command_timeout_is_parsed_from_the_environment() {
    let env = ConfigOverrides::from_env([("MOMO_COMMAND_TIMEOUT", "5")]).expect("a valid timeout");
    let config = Config::resolve(ConfigOverrides::default(), env);
    assert_eq!(config.command_timeout, Duration::from_secs(5));
}

#[test]
fn a_non_positive_command_timeout_is_rejected() {
    let file = "command_timeout = 0\n"
        .parse::<ConfigOverrides>()
        .expect_err("zero seconds is not a timeout");
    assert!(file.to_string().contains("command_timeout"), "{file}");

    let env = ConfigOverrides::from_env([("MOMO_COMMAND_TIMEOUT", "-1")])
        .expect_err("a negative timeout is rejected");
    assert!(env.to_string().contains("MOMO_COMMAND_TIMEOUT"), "{env}");
}

#[test]
fn a_non_integer_command_timeout_is_rejected() {
    let err = "command_timeout = \"soon\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("a string is not a number of seconds");
    assert!(err.to_string().contains("command_timeout"), "{err}");
}

#[test]
fn a_non_default_command_timeout_is_rendered_and_round_trips() {
    assert!(
        !Config::default().to_string().contains("command_timeout"),
        "the default timeout is not written"
    );

    let config = Config {
        command_timeout: Duration::from_secs(5),
        ..Config::default()
    };
    let rendered = config.to_string();
    assert!(rendered.contains("command_timeout = 5"), "{rendered}");

    let parsed: ConfigOverrides = rendered.parse().expect("generated config is valid TOML");
    let resolved = Config::resolve(parsed, ConfigOverrides::default());
    assert_eq!(resolved.command_timeout, Duration::from_secs(5));
}

#[test]
fn unknown_env_vars_are_ignored() {
    let env = ConfigOverrides::from_env([("MOMO_UNRELATED", "whatever")])
        .expect("unknown names are ignored");
    assert_eq!(env, ConfigOverrides::default());
}

#[test]
fn unknown_env_value_is_rejected() {
    let err = ConfigOverrides::from_env([("MOMO_RECORDING", "nonsense")])
        .expect_err("unknown recording value fails");
    assert!(err.to_string().contains("MOMO_RECORDING"));
}

#[test]
fn an_empty_env_value_is_treated_as_unset() {
    let env = ConfigOverrides::from_env([
        ("MOMO_TAG_PREFIX", "rel-"),
        ("MOMO_CHANGELOG", ""),
        ("MOMO_CHANGES_DIR", ""),
        ("MOMO_TASK_ENV", ""),
    ])
    .expect("empty values are ignored");
    assert_eq!(env.tag_prefix.as_deref(), Some("rel-"));
    assert_eq!(env.changelog, None);
    assert_eq!(env.changes_dir, None);
    assert_eq!(env.task_env, None);
}

#[test]
fn an_absolute_path_in_the_environment_is_rejected() {
    let err = ConfigOverrides::from_env([("MOMO_CHANGELOG", "/tmp/x.md")])
        .expect_err("an absolute path fails");
    let message = err.to_string();
    assert!(message.contains("MOMO_CHANGELOG"), "{message}");
    assert!(message.contains("/tmp/x.md"), "{message}");
}

#[test]
fn packages_are_parsed_and_layered() {
    let file: ConfigOverrides = "\
[packages.\"cargo:momo\"]
slug = \"cargo-momo\"
"
    .parse()
    .expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    let package = config.packages.get("cargo:momo").expect("package present");
    assert_eq!(package.slug.as_deref(), Some("cargo-momo"));
    assert_eq!(package.identity, None);
    assert_eq!(package.manifest, None);
    assert_eq!(package.tag_prefix, None);
}

#[test]
fn a_package_tag_prefix_override_is_parsed() {
    let file: ConfigOverrides = "\
[packages.\"go:example.com/acme/api\"]
tag_prefix = \"api/v\"
"
    .parse()
    .expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    let package = config
        .packages
        .get("go:example.com/acme/api")
        .expect("package present");
    assert_eq!(package.tag_prefix.as_deref(), Some("api/v"));
}

#[test]
fn a_package_manifest_override_is_parsed() {
    let file: ConfigOverrides = "\
[packages.\"cargo:momo\"]
manifest = \"crates/momo/Cargo.toml\"
"
    .parse()
    .expect("valid TOML");
    let config = Config::resolve(file, ConfigOverrides::default());
    let package = config.packages.get("cargo:momo").expect("package present");
    assert_eq!(
        package.manifest.as_deref(),
        Some(Path::new("crates/momo/Cargo.toml"))
    );
}

#[test]
fn generated_config_round_trips() {
    let mut original = Config {
        remote: Some(
            "https://github.com/Jarvvski/momo"
                .parse()
                .expect("a remote URL"),
        ),
        task_env: Some("mise".to_owned()),
        rc_format: Some("-rc.{n}".to_owned()),
        ..Config::default()
    };
    original.packages.insert(
        "cargo:momo".to_owned(),
        PackageConfig {
            identity: None,
            slug: Some("cargo-momo".to_owned()),
            manifest: Some(PathBuf::from("crates/momo/Cargo.toml")),
            tag_prefix: Some("cargo-momo-v".to_owned()),
        },
    );

    let rendered = original.to_string();
    let parsed: ConfigOverrides = rendered.parse().expect("generated config is valid TOML");
    let resolved = Config::resolve(parsed, ConfigOverrides::default());
    assert_eq!(resolved, original);
}

#[test]
fn wrong_type_is_rejected() {
    let err = "recording = 3\n"
        .parse::<ConfigOverrides>()
        .expect_err("a number is not a string");
    assert!(err.to_string().contains("recording"));
}

#[test]
fn invalid_toml_is_rejected() {
    let err = "recording = \n"
        .parse::<ConfigOverrides>()
        .expect_err("invalid TOML fails");
    assert!(err.to_string().contains("invalid TOML"));
}

#[test]
fn an_unknown_top_level_key_is_rejected() {
    let err = "recordingg = \"fragments\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("an unknown key fails");
    assert!(err.to_string().contains("recordingg"), "{err}");
}

#[test]
fn an_empty_path_in_the_file_is_rejected() {
    let err = "changelog = \"\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("an empty path fails");
    assert!(err.to_string().contains("changelog"), "{err}");
}

#[test]
fn an_absolute_path_in_the_file_is_rejected() {
    let err = "changes_dir = \"/tmp/x\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("an absolute path fails");
    let message = err.to_string();
    assert!(message.contains("changes_dir"), "{message}");
    assert!(message.contains("/tmp/x"), "{message}");
}

#[test]
fn a_parent_escaping_path_in_the_file_is_rejected() {
    let err = "changelog = \"../x.md\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("a parent-escaping path fails");
    assert!(err.to_string().contains("changelog"), "{err}");
}

#[test]
fn an_ordinary_relative_path_still_parses() {
    let file: ConfigOverrides = "changelog = \"docs/CHANGELOG.md\"\n"
        .parse()
        .expect("a relative path parses");
    assert_eq!(
        file.changelog.as_deref(),
        Some(Path::new("docs/CHANGELOG.md"))
    );
}

#[test]
fn an_unknown_package_key_is_rejected() {
    let err = "[packages.\"cargo:momo\"]\nslgu = \"x\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("an unknown package key fails");
    assert!(
        err.to_string().contains("packages.cargo:momo.slgu"),
        "{err}"
    );
}

#[test]
fn a_parent_escaping_manifest_is_rejected() {
    let err = "[packages.\"cargo:momo\"]\nmanifest = \"a/../b\"\n"
        .parse::<ConfigOverrides>()
        .expect_err("a parent-escaping manifest fails");
    assert!(err.to_string().contains("manifest"), "{err}");
}

#[test]
fn with_task_env_sets_the_key_and_preserves_the_rest() {
    let original = "# my config\nrecording = \"fragments\" # keep me\n";
    let updated = with_task_env(original, Some("mise")).expect("valid TOML");
    assert!(updated.contains("# my config"), "{updated}");
    assert!(
        updated.contains("recording = \"fragments\" # keep me"),
        "{updated}"
    );
    assert!(updated.contains("task_env = \"mise\""), "{updated}");
}

#[test]
fn with_task_env_none_removes_the_key() {
    let original = "task_env = \"mise\"\nrecording = \"changelog\"\n";
    let updated = with_task_env(original, None).expect("valid TOML");
    assert!(!updated.contains("task_env"), "{updated}");
    assert!(updated.contains("recording = \"changelog\""), "{updated}");
}

#[test]
fn with_task_env_is_idempotent() {
    let once = with_task_env("recording = \"changelog\"\n", Some("mise")).expect("valid TOML");
    let twice = with_task_env(&once, Some("mise")).expect("valid TOML");
    assert_eq!(once, twice);
}
