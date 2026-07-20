use std::path::PathBuf;

use rayon::prelude::*;

use crate::{discover, editorconfig, ui};
use prim_fmt::{FileKind, Style};

pub(super) type FormattedFile = (PathBuf, FileKind, Style, bool, String, String);

#[derive(Debug, PartialEq, Eq)]
enum LoadMessageKind {
    Warning,
    Error,
}

#[derive(Debug, PartialEq, Eq)]
struct LoadMessage {
    kind: LoadMessageKind,
    text: String,
}

enum LoadOutcome {
    Formatted(FormattedFile),
    Message(LoadMessage),
    Skipped,
}

pub(super) fn load_and_format(
    paths: &[PathBuf],
    excludes: &[String],
    respect_vcs_ignore: bool,
) -> Result<(Vec<FormattedFile>, bool), ignore::Error> {
    let files = discover::collect(paths, excludes, respect_vcs_ignore)?;
    let outcomes = load_discovered(files);
    let (results, messages, had_error) = summarize_outcomes(outcomes);
    emit_messages(&messages);
    Ok((results, had_error))
}

fn load_discovered(files: Vec<discover::Discovered>) -> Vec<LoadOutcome> {
    files
        .into_par_iter()
        .map_init(editorconfig::Resolver::new, |resolver, file| {
            load_one(resolver, file)
        })
        .collect()
}

fn load_one(resolver: &mut editorconfig::Resolver, file: discover::Discovered) -> LoadOutcome {
    let Some(kind) = prim_fmt::classify(&file.path) else {
        return if file.explicit {
            if file.path.exists() {
                warning(format!(
                    "{}: not a file type prim formats; skipped",
                    file.path.display()
                ))
            } else {
                error(format!("{}: no such file", file.path.display()))
            }
        } else {
            LoadOutcome::Skipped
        };
    };

    let original = match std::fs::read_to_string(&file.path) {
        Ok(text) => text,
        Err(err) => {
            let message = format!("{}: {err}", file.path.display());
            return if file.explicit {
                error(message)
            } else {
                warning(message)
            };
        }
    };

    let style = resolver.resolve(&file.path);
    let formatted = match prim_fmt::format(kind, &original, &style) {
        Ok(text) => text,
        Err(err) => {
            let message = format!("{}: {err}", file.path.display());
            return if file.explicit {
                error(message)
            } else {
                warning(message)
            };
        }
    };

    let markdown_strict = if kind == FileKind::Markdown {
        resolver.resolve_mdlint_strict(&file.path)
    } else {
        false
    };

    LoadOutcome::Formatted((file.path, kind, style, markdown_strict, original, formatted))
}

fn summarize_outcomes(outcomes: Vec<LoadOutcome>) -> (Vec<FormattedFile>, Vec<LoadMessage>, bool) {
    let mut had_error = false;
    let mut messages = Vec::new();
    let mut results = Vec::new();

    for outcome in outcomes {
        match outcome {
            LoadOutcome::Formatted(file) => results.push(file),
            LoadOutcome::Message(message) => {
                had_error |= message.kind == LoadMessageKind::Error;
                messages.push(message);
            }
            LoadOutcome::Skipped => {}
        }
    }

    (results, messages, had_error)
}

fn emit_messages(messages: &[LoadMessage]) {
    for message in messages {
        match message.kind {
            LoadMessageKind::Warning => ui::warning(&message.text),
            LoadMessageKind::Error => ui::error(&message.text),
        }
    }
}

fn warning(text: String) -> LoadOutcome {
    LoadOutcome::Message(LoadMessage {
        kind: LoadMessageKind::Warning,
        text,
    })
}

fn error(text: String) -> LoadOutcome {
    LoadOutcome::Message(LoadMessage {
        kind: LoadMessageKind::Error,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::ThreadPoolBuilder;
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn write_bytes(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn uneven_json(item_count: usize) -> String {
        let items = (0..item_count)
            .map(|index| format!("\"item-{index}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{\"items\":[{items}]}}\n")
    }

    fn file_names(results: &[FormattedFile]) -> Vec<String> {
        results
            .iter()
            .map(|file| file.0.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn formatted_results_preserve_discovery_order_with_uneven_work_sizes() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("b.json"), &uneven_json(8));
        write(&dir.path().join("a.json"), &uneven_json(2_000));
        write(&dir.path().join("c.json"), &uneven_json(40));

        let discovered = discover::collect(&[dir.path().to_path_buf()], &[], true).unwrap();
        let expected = discovered
            .iter()
            .map(|file| {
                file.path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();

        let outcomes = ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| load_discovered(discovered));
        let (results, messages, had_error) = summarize_outcomes(outcomes);

        assert_eq!(file_names(&results), expected);
        assert!(messages.is_empty());
        assert!(!had_error);
    }

    #[test]
    fn load_messages_preserve_discovery_order_for_warnings_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("b.json");
        write_bytes(&dir.path().join("a.json"), &[0xFF, 0xFE, 0x00, 0x01]);

        let discovered =
            discover::collect(&[dir.path().to_path_buf(), missing.clone()], &[], true).unwrap();
        let outcomes = ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| load_discovered(discovered));
        let (_results, messages, had_error) = summarize_outcomes(outcomes);

        assert!(had_error);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].kind, LoadMessageKind::Warning);
        assert!(
            messages[0]
                .text
                .starts_with(&format!("{}:", dir.path().join("a.json").display()))
        );
        assert_eq!(messages[1].kind, LoadMessageKind::Error);
        assert!(
            messages[1]
                .text
                .starts_with(&format!("{}:", missing.display()))
        );
    }
}
