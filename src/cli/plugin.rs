//! CLI plugin management: `par-term plugin add|update|remove`.
//!
//! Prompt-confirm-execute-report wrappers around the headless git layer in
//! [`par_term_scripting::plugin_git`], which owns every git invocation. The
//! prompts are the procedural half of the plugin trust model: add warns
//! before cloning anything, and update shows a diff before fast-forwarding,
//! so no upstream code lands without the user seeing it happen. Like
//! [`super::install`], these subcommands run and exit before the application
//! starts, which is why blocking stdin reads are safe here.

use std::io::{self, Write};
use std::path::Path;

use clap::Subcommand;

use par_term_scripting::manifest::discover_plugins;
use par_term_scripting::plugin_git;

/// `par-term plugin <command>` — see `Commands::Plugin` in [`super`].
#[derive(Subcommand)]
pub enum PluginCommands {
    /// Install a plugin from a git URL; it lands DISABLED
    Add {
        /// Git URL to clone from (https://, ssh://, git://, file://, scp-like)
        url: String,

        /// Skip the confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Fetch upstream and fast-forward installed plugins after showing a diff
    Update {
        /// Plugin id; updates every git-installed plugin when omitted
        id: Option<String>,

        /// Skip the per-plugin confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Remove a plugin that was installed by `plugin add`
    Remove {
        /// Plugin id to remove
        id: String,

        /// Skip the confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

/// `par-term plugin add <url>`
pub fn plugin_add_cli(url: &str, yes: bool) -> anyhow::Result<()> {
    let root = crate::config::Config::config_dir().join("plugins");
    println!("Plugins directory: {}", root.display());
    println!();
    println!("About to clone a plugin from:");
    println!("  {url}");
    println!();
    println!("Plugins run as supervised subprocesses of par-term. This one is");
    println!("installed DISABLED — read its code before enabling it in");
    println!("Settings > Automation > Plugins.");
    if !yes && !confirm("Clone and install? [y/N] ")? {
        println!("Cancelled.");
        return Ok(());
    }
    println!();
    match plugin_git::add(&root, url) {
        Ok(plugin) => {
            let manifest = &plugin.manifest;
            println!(
                "Installed {} {} ({})",
                manifest.name, manifest.version, manifest.id
            );
            println!("  {}", root.join(&manifest.id).display());
            println!();
            println!("It is DISABLED. Read the code, then enable it in");
            println!("Settings > Automation > Plugins.");
            Ok(())
        }
        Err(reason) => {
            eprintln!("Error: {reason}");
            Err(anyhow::anyhow!(reason))
        }
    }
}

/// `par-term plugin update [id]`
pub fn plugin_update_cli(id: Option<&str>, yes: bool) -> anyhow::Result<()> {
    let root = crate::config::Config::config_dir().join("plugins");
    let targets: Vec<String> = match id {
        Some(one) => vec![one.to_string()],
        None => {
            let (discovered, _) = discover_plugins(&root);
            let mut ids: Vec<String> = discovered
                .into_iter()
                .map(|plugin| plugin.manifest.id)
                .filter(|pid| plugin_git::is_git_installed(&root.join(pid)))
                .collect();
            ids.sort();
            if ids.is_empty() {
                println!("No git-installed plugins under {}.", root.display());
                println!("Plugins installed by copying a directory by hand are updated");
                println!("the same way — `plugin update` only manages what it installed.");
                return Ok(());
            }
            ids
        }
    };

    let mut failures = 0usize;
    for pid in targets {
        println!("=== {pid} ===");
        if let Err(reason) = update_one(&root, &pid, yes) {
            eprintln!("Error: {reason}");
            failures += 1;
        }
        println!();
    }
    if failures > 0 {
        return Err(anyhow::anyhow!("{failures} plugin update(s) failed"));
    }
    Ok(())
}

fn update_one(root: &Path, pid: &str, yes: bool) -> anyhow::Result<()> {
    let dir = root.join(pid);
    let preview = plugin_git::update_fetch(&dir).map_err(|reason| anyhow::anyhow!(reason))?;
    if preview.up_to_date {
        println!("Already up to date ({}).", preview.current);
        return Ok(());
    }
    println!("{} -> {}", preview.current, preview.incoming);
    println!();
    println!("{}", preview.diff);
    println!();
    if !yes && !confirm("Apply this update? [y/N] ")? {
        println!("Skipped.");
        return Ok(());
    }
    let new_head = plugin_git::update_apply(&dir).map_err(|reason| anyhow::anyhow!(reason))?;
    println!("Updated to {new_head}.");
    Ok(())
}

/// `par-term plugin remove <id>`
pub fn plugin_remove_cli(id: &str, yes: bool) -> anyhow::Result<()> {
    let root = crate::config::Config::config_dir().join("plugins");
    println!("About to remove:");
    println!("  {}", root.join(id).display());
    println!();
    if !yes && !confirm("Remove this plugin? [y/N] ")? {
        println!("Cancelled.");
        return Ok(());
    }
    match plugin_git::remove(&root, id) {
        Ok(()) => {
            println!("Removed {id}.");
            Ok(())
        }
        Err(reason) => {
            eprintln!("Error: {reason}");
            Err(anyhow::anyhow!(reason))
        }
    }
}

fn confirm(prompt: &str) -> anyhow::Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    let response = response.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}
