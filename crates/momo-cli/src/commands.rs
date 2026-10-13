//! Command handlers.
//!
//! Only `init` has a real handler in v0; every other verb is declared on the
//! command surface and exits non-zero with a "not implemented" message.

use std::env;
use std::fs;

use anyhow::{Context, Result, bail};

use crate::cli::{Cli, Command};
use crate::config::{self, CONFIG_FILE};

/// Dispatch a parsed command line.
pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Init => init(cli),
        other => bail!("`momo {other}` is not implemented yet"),
    }
}

/// Scaffold `momo.toml` at the project root.
///
/// Writes the effective configuration. With `--dry-run`, prints what would be
/// written and leaves the tree untouched.
fn init(cli: &Cli) -> Result<()> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    let root = config::project_root(&cwd);
    let path = root.join(CONFIG_FILE);

    if cli.dry_run {
        let config = config::load(&root)?;
        eprintln!("would write {}", path.display());
        print!("{config}");
        return Ok(());
    }

    if path.exists() {
        bail!("{} already exists; refusing to overwrite", path.display());
    }

    let config = config::load(&root)?;
    fs::write(&path, config.to_string()).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}
