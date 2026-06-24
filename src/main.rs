mod ast;
mod cli;
mod config;
pub mod inventory;
pub mod languages;
pub mod processor;
mod rules;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands, InventoryArgs};
use config::ConfigManager;
use glob::glob;
use once_cell::sync::Lazy;
use processor::OutputWriter;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

static DEFAULT_LANGUAGE_REGISTRY: Lazy<languages::LanguageRegistry> =
    Lazy::new(languages::LanguageRegistry::new);

#[derive(Debug, Default)]
struct UnsupportedFilesReport {
    total: usize,
    by_extension: std::collections::BTreeMap<String, usize>,
    samples: Vec<PathBuf>,
}

type ImportantRemovalSample = (Arc<PathBuf>, processor::ImportantRemoval);

fn main() -> Result<()> {
    #[cfg(unix)]
    unsafe {
        // Avoid panicking on broken pipes (e.g. `uncomment ... | head`) by restoring
        // SIGPIPE default behavior.
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();

    if let Some(command) = cli.command {
        return match command {
            Commands::Init {
                output,
                force,
                comprehensive,
                interactive,
            } => Cli::handle_init_command(&output, force, comprehensive, interactive),
            Commands::Inventory(args) => run_inventory(args),
        };
    }

    let options = cli.args.processing_options();

    if cli.args.paths.is_empty() {
        eprintln!("Error: No input paths specified. Use 'uncomment --help' for usage information.");
        std::process::exit(1);
    }

    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    let config_manager = if let Some(config_path) = &cli.args.config {
        let config = config::Config::from_file(config_path)
            .with_context(|| format!("Failed to load config file: {}", config_path.display()))?;

        ConfigManager::from_single_config(current_dir, config)?
    } else {
        ConfigManager::new(&current_dir).context("Failed to initialize configuration manager")?
    };

    let mut unsupported_report = UnsupportedFilesReport::default();
    let files = collect_files(&cli.args.paths, &options, &mut unsupported_report)?;

    print_unsupported_files_report(&unsupported_report, cli.args.verbose);

    if files.is_empty() {
        eprintln!("No supported files found to process in the specified paths.");
        eprintln!("{}", supported_extensions_message());
        if options.respect_gitignore {
            eprintln!("Tip: Use --no-gitignore to process files ignored by git.");
        }
        return Ok(());
    }

    let num_threads = if cli.args.threads == 0 {
        num_cpus::get()
    } else {
        cli.args.threads
    };

    if cli.args.verbose && num_threads > 1 {
        println!("🔧 Using {num_threads} parallel threads");
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .context("Failed to initialize thread pool")?;

    let output_writer = Arc::new(OutputWriter::new(
        options.dry_run,
        cli.args.verbose,
        options.dry_run && options.show_diff,
    ));

    let total_files = files.len();

    // Process files in parallel (or sequentially if threads=1), collecting results
    // with zero mutex contention on the hot path.
    let process_file = |file_path: &PathBuf| -> Option<processor::ProcessedFile> {
        let mut proc = processor::Processor::new_with_config(&config_manager);
        match proc.process_file_with_config(file_path, &config_manager, Some(&options)) {
            Ok(mut pf) => {
                pf.modified = pf.original_content != pf.processed_content;
                Some(pf)
            }
            Err(e) => {
                eprintln!("Error processing {}: {e}", file_path.display());
                if cli.args.verbose {
                    eprintln!("  Full error: {e:?}");
                }
                None
            }
        }
    };

    let results: Vec<processor::ProcessedFile> = if num_threads == 1 {
        files.iter().filter_map(process_file).collect()
    } else {
        files.par_iter().filter_map(process_file).collect()
    };

    // Sequential phase: write output, collect stats
    let mut modified_files = 0usize;
    let mut important_removal_count = 0usize;
    let mut important_removal_samples: Vec<ImportantRemovalSample> = Vec::new();

    for processed_file in &results {
        if processed_file.modified {
            modified_files += 1;
        }

        if !processed_file.important_removals.is_empty() {
            important_removal_count += processed_file.important_removals.len();
            const MAX_SAMPLES: usize = 20;
            let remaining = MAX_SAMPLES.saturating_sub(important_removal_samples.len());
            if remaining > 0 {
                let sample_path = Arc::new(processed_file.path.clone());
                for removal in processed_file.important_removals.iter().take(remaining) {
                    important_removal_samples.push((Arc::clone(&sample_path), removal.clone()));
                }
            }
        }

        output_writer.write_file(processed_file)?;
    }

    output_writer.print_summary(total_files, modified_files);

    if important_removal_count > 0 {
        eprintln!(
            "Warning: removed {important_removal_count} potentially important comment(s). Re-run with `--dry-run --diff` to inspect."
        );
        if cli.args.verbose {
            eprintln!("Examples:");
            for (path, removal) in &important_removal_samples {
                eprintln!(
                    "  - {}:{} [{}] {}",
                    path.display(),
                    removal.line,
                    removal.reason,
                    removal.preview
                );
            }
        }
    }

    Ok(())
}

fn run_inventory(args: InventoryArgs) -> Result<()> {
    if args.paths.is_empty() {
        eprintln!("Error: No input paths specified for inventory.");
        std::process::exit(1);
    }

    let mut options = args.into_options();
    apply_inventory_config_defaults(&mut options);
    let result = inventory::run(&options).context("inventory run failed")?;

    println!(
        "Inventory: {} record(s) from {} file(s) ({} with records) -> {}",
        result.records,
        result.files_scanned,
        result.files_with_records,
        options.output_dir.display()
    );
    if result.suppressed > 0 {
        println!(
            "Suppressed {} generated/vendor record(s) (use --include-generated to keep).",
            result.suppressed
        );
    }
    for error in &result.errors {
        eprintln!("inventory warning: {error}");
    }

    Ok(())
}

/// Apply `[inventory]` config defaults beneath CLI flags.
///
/// CLI flags take precedence; config only fills fields still at their built-in
/// default value (configuration-system precedence rule).
fn apply_inventory_config_defaults(options: &mut inventory::InventoryOptions) {
    use inventory::{InventoryFormat, InventoryOptions};

    let local = std::path::Path::new(".uncommentrc.toml");
    let Ok(text) = std::fs::read_to_string(local) else {
        return;
    };
    let Ok(config) = toml::from_str::<config::Config>(&text) else {
        return;
    };
    let Some(inv) = config.inventory else {
        return;
    };

    let defaults = InventoryOptions::default();

    if options.output_dir == defaults.output_dir
        && let Some(dir) = inv.output_dir
    {
        options.output_dir = std::path::PathBuf::from(dir);
    }
    if options.formats == defaults.formats && !inv.formats.is_empty() {
        let parsed: Vec<InventoryFormat> = inv
            .formats
            .iter()
            .filter_map(|f| match f.as_str() {
                "jsonl" => Some(InventoryFormat::Jsonl),
                "markdown" => Some(InventoryFormat::Markdown),
                "summary" => Some(InventoryFormat::Summary),
                "by-symbol" => Some(InventoryFormat::BySymbol),
                _ => None,
            })
            .collect();
        if !parsed.is_empty() {
            options.formats = parsed;
        }
    }
    if options.languages == defaults.languages && !inv.languages.is_empty() {
        options.languages = inv.languages;
    }
    if options.min_priority == defaults.min_priority
        && let Some(p) = inv.min_priority.as_deref()
    {
        options.min_priority = match p {
            "ignore" => inventory::model::Priority::Ignore,
            "low" => inventory::model::Priority::Low,
            "medium" => inventory::model::Priority::Medium,
            "high" => inventory::model::Priority::High,
            _ => options.min_priority,
        };
    }
    if inv.include_generated {
        options.include_generated = true;
    }
}

fn collect_files(
    paths: &[String],
    options: &processor::ProcessingOptions,
    unsupported: &mut UnsupportedFilesReport,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            if has_supported_extension(path) {
                files.push(path.to_path_buf());
            } else {
                record_unsupported_file(path, unsupported);
            }
        } else if path.is_dir() {
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, options, unsupported)?
        } else {
            collect_from_pattern(path_pattern, &mut files, options, unsupported)?
        }
    }

    files.sort();
    files.dedup();

    Ok(files)
}

fn collect_from_pattern(
    pattern: &str,
    files: &mut Vec<PathBuf>,
    options: &processor::ProcessingOptions,
    unsupported: &mut UnsupportedFilesReport,
) -> Result<()> {
    if options.respect_gitignore {
        use ignore::WalkBuilder;
        use std::path::PathBuf;

        let pattern_path = if pattern.contains("/**/*") {
            pattern.strip_suffix("/**/*").unwrap_or(".")
        } else {
            pattern
        };

        let pattern_path_buf = PathBuf::from(pattern_path);
        let absolute_pattern = if pattern_path_buf.is_absolute() {
            pattern_path_buf
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&pattern_path_buf)
        };

        let mut git_root = None;
        let mut current = absolute_pattern.as_path();
        while let Some(parent) = current.parent() {
            if parent.join(".git").exists() {
                git_root = Some(parent.to_path_buf());
                break;
            }
            current = parent;
        }

        let (walk_root, filter_prefix) = if let Some(root) = git_root {
            if absolute_pattern.starts_with(&root) {
                (root, Some(absolute_pattern))
            } else {
                (absolute_pattern, None)
            }
        } else {
            (absolute_pattern, None)
        };

        let walker = WalkBuilder::new(walk_root)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .parents(true)
            .require_git(false)
            .build();

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    if let Some(ref prefix) = filter_prefix
                        && !path.starts_with(prefix)
                    {
                        continue;
                    }

                    if path.is_file() {
                        if has_supported_extension(path) {
                            files.push(path.to_path_buf());
                        } else {
                            record_unsupported_file(path, unsupported);
                        }
                    }
                }
                Err(e) => eprintln!("Error reading path: {e}"),
            }
        }
    } else {
        for entry in glob(pattern).context("Failed to parse glob pattern")? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        if has_supported_extension(&path) {
                            files.push(path);
                        } else {
                            record_unsupported_file(&path, unsupported);
                        }
                    }
                }
                Err(e) => eprintln!("Error reading path: {e}"),
            }
        }
    }
    Ok(())
}

fn record_unsupported_file(path: &Path, report: &mut UnsupportedFilesReport) {
    report.total += 1;

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))
        .unwrap_or_else(|| "<no extension>".to_string());

    *report.by_extension.entry(extension).or_insert(0) += 1;

    const MAX_SAMPLES: usize = 10;
    if report.samples.len() < MAX_SAMPLES {
        report.samples.push(path.to_path_buf());
    }
}

fn print_unsupported_files_report(report: &UnsupportedFilesReport, verbose: bool) {
    if report.total == 0 {
        return;
    }

    let mut top: Vec<(&String, &usize)> = report.by_extension.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    const MAX_TOP: usize = 8;
    let shown = top.into_iter().take(MAX_TOP).collect::<Vec<_>>();
    let mut summary = String::new();
    for (i, (ext, count)) in shown.iter().enumerate() {
        if i > 0 {
            summary.push_str(", ");
        }
        summary.push_str(&format!("{ext}={count}"));
    }

    eprintln!("Skipping {} unsupported file(s) ({summary}).", report.total);

    if verbose && !report.samples.is_empty() {
        eprintln!("Examples:");
        for sample in &report.samples {
            eprintln!("  - {}", sample.display());
        }
    }
}

fn has_supported_extension(path: &Path) -> bool {
    DEFAULT_LANGUAGE_REGISTRY.detect_language(path).is_some()
}

fn supported_extensions_message() -> String {
    let mut extensions: Vec<String> = DEFAULT_LANGUAGE_REGISTRY
        .get_supported_extensions()
        .into_iter()
        .map(|ext| format!(".{ext}"))
        .collect();
    extensions.push(".d.ts".to_string());
    extensions.sort();
    extensions.dedup();

    let mut shown = extensions;
    shown.sort();

    // Keep the message readable.
    const MAX: usize = 20;
    if shown.len() > MAX {
        shown.truncate(MAX);
        format!("Supported extensions: {}, and more.", shown.join(", "))
    } else {
        format!("Supported extensions: {}.", shown.join(", "))
    }
}
