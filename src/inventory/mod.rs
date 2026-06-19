//! Comment inventory mode: a non-destructive parallel pipeline that extracts
//! comments, docstrings, and YAML/Hydra config facts, attaches context,
//! classifies and scores them, and writes JSONL/Markdown/summary artifacts.
//!
//! This path never mutates source files (NFR-002) and is fully additive to the
//! existing removal pipeline (FR-006).

pub mod c_family;
pub mod collector;
pub mod ids;
pub mod model;
pub mod python;
pub mod scoring;
pub mod source_class;
pub mod text;
pub mod writers;
pub mod yaml;

use crate::inventory::model::{InventoryRecord, Priority};
use crate::inventory::source_class::FileOrigin;
use crate::languages::LanguageRegistry;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Output artifact formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum InventoryFormat {
    Jsonl,
    Markdown,
    Summary,
    #[value(name = "by-symbol")]
    BySymbol,
}

impl InventoryFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            InventoryFormat::Jsonl => "jsonl",
            InventoryFormat::Markdown => "markdown",
            InventoryFormat::Summary => "summary",
            InventoryFormat::BySymbol => "by-symbol",
        }
    }
}

/// Resolved inventory run options (CLI flags merged over config defaults).
#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub paths: Vec<String>,
    pub output_dir: PathBuf,
    pub formats: Vec<InventoryFormat>,
    pub languages: Vec<String>,
    pub min_priority: Priority,
    pub include_generated: bool,
    pub include_low_priority: bool,
    pub include_yaml_values: bool,
    pub respect_gitignore: bool,
    pub threads: usize,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            output_dir: PathBuf::from(".agent/inventory"),
            formats: vec![
                InventoryFormat::Jsonl,
                InventoryFormat::Markdown,
                InventoryFormat::Summary,
            ],
            languages: vec!["python".into(), "c".into(), "cpp".into(), "yaml".into()],
            min_priority: Priority::Low,
            include_generated: false,
            include_low_priority: false,
            include_yaml_values: true,
            respect_gitignore: true,
            threads: 1,
        }
    }
}

/// Outcome of an inventory run.
#[derive(Debug)]
pub struct InventoryResult {
    pub records: usize,
    pub files_scanned: usize,
    pub files_with_records: usize,
    pub suppressed: usize,
    pub errors: Vec<String>,
}

/// Execute inventory mode end-to-end.
pub fn run(options: &InventoryOptions) -> Result<InventoryResult> {
    let registry = LanguageRegistry::new();
    let requested: BTreeSet<String> = options.languages.iter().map(|l| l.to_lowercase()).collect();

    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let files = discover_files(
        &options.paths,
        options.respect_gitignore,
        &registry,
        &requested,
    )?;
    let files_scanned = files.len();

    configure_threads(options.threads);

    let collected: Vec<(Vec<InventoryRecord>, Option<String>)> = files
        .par_iter()
        .map(|(path, language, tslp)| collect_one(path, language, tslp, &root, options))
        .collect();

    let mut all: Vec<InventoryRecord> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (records, err) in collected {
        all.extend(records);
        if let Some(e) = err {
            errors.push(e);
        }
    }

    // Deterministic ordering (FR-012).
    all.sort_by_key(|r| r.sort_key());

    let (kept, suppressed) = filter_records(all, options);

    let files_with_records = kept
        .iter()
        .map(|r| r.path.clone())
        .collect::<BTreeSet<_>>()
        .len();

    let manifest = writers::Manifest::new(
        root.display().to_string(),
        options.paths.clone(),
        options
            .formats
            .iter()
            .map(|f| f.as_str().to_string())
            .collect(),
        options.languages.clone(),
        kept.len(),
        files_scanned,
        files_with_records,
        errors.clone(),
    );

    writers::write_outputs(
        &kept,
        &options.formats,
        suppressed,
        &options.output_dir,
        &manifest,
    )?;

    Ok(InventoryResult {
        records: kept.len(),
        files_scanned,
        files_with_records,
        suppressed,
        errors,
    })
}

fn collect_one(
    path: &Path,
    language: &str,
    tslp: &str,
    root: &Path,
    options: &InventoryOptions,
) -> (Vec<InventoryRecord>, Option<String>) {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return (Vec::new(), Some(format!("{}: {e}", path.display()))),
    };
    let rel = relative_path(root, path);
    let repo = repo_name(root);
    match collector::collect_file(
        &repo,
        &rel,
        &source,
        language,
        tslp,
        options.include_yaml_values,
    ) {
        Ok(records) => (records, None),
        Err(e) => (Vec::new(), Some(format!("{}: {e}", path.display()))),
    }
}

fn filter_records(
    all: Vec<InventoryRecord>,
    options: &InventoryOptions,
) -> (Vec<InventoryRecord>, usize) {
    let min = if options.include_low_priority {
        Priority::Ignore
    } else {
        options.min_priority
    };
    let min_rank = min.rank();

    let mut suppressed = 0usize;
    let kept: Vec<InventoryRecord> = all
        .into_iter()
        .filter(|r| {
            let generated = r
                .file_origin
                .as_deref()
                .map(|o| FileOrigin::from_label(o).is_generated_or_vendor())
                .unwrap_or(false);
            if generated && !options.include_generated && r.priority != Priority::High {
                suppressed += 1;
                return false;
            }
            if r.priority.rank() < min_rank {
                if generated {
                    suppressed += 1;
                }
                return false;
            }
            true
        })
        .collect();

    (kept, suppressed)
}

fn configure_threads(threads: usize) {
    let n = if threads == 0 {
        num_cpus::get()
    } else {
        threads
    };
    if n > 1 {
        // Ignore error if a global pool already exists.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }
}

/// Discover supported files for the requested languages.
fn discover_files(
    paths: &[String],
    respect_gitignore: bool,
    registry: &LanguageRegistry,
    requested: &BTreeSet<String>,
) -> Result<Vec<(PathBuf, String, String)>> {
    let mut out: Vec<(PathBuf, String, String)> = Vec::new();

    for raw in paths {
        let path = Path::new(raw);
        if path.is_file() {
            push_if_match(path, registry, requested, &mut out);
        } else if path.is_dir() {
            walk_dir(path, respect_gitignore, registry, requested, &mut out)?;
        } else {
            // Glob pattern.
            for entry in glob::glob(raw).context("invalid glob pattern")?.flatten() {
                if entry.is_file() {
                    push_if_match(&entry, registry, requested, &mut out);
                }
            }
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

fn walk_dir(
    dir: &Path,
    respect_gitignore: bool,
    registry: &LanguageRegistry,
    requested: &BTreeSet<String>,
    out: &mut Vec<(PathBuf, String, String)>,
) -> Result<()> {
    use ignore::WalkBuilder;
    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .parents(respect_gitignore)
        .require_git(false)
        .build();
    for entry in walker {
        match entry {
            Ok(e) if e.path().is_file() => push_if_match(e.path(), registry, requested, out),
            Ok(_) => {}
            Err(e) => eprintln!("inventory: error reading path: {e}"),
        }
    }
    Ok(())
}

fn push_if_match(
    path: &Path,
    registry: &LanguageRegistry,
    requested: &BTreeSet<String>,
    out: &mut Vec<(PathBuf, String, String)>,
) {
    if let Some(config) = registry.detect_language_arc(path) {
        let name = config.name.to_lowercase();
        if requested.contains(&name) {
            out.push((path.to_path_buf(), name, config.tslp_name.clone()));
        }
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn repo_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string())
}
