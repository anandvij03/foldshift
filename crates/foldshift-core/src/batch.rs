use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::{FcError, Result};

/// A thread-safe cooperative cancellation token.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal cancellation to all workers listening to this token.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Return an error if cancellation was requested.
    pub fn check_cancelled(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(FcError::Conversion(
                "operation was cancelled by user".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Status of an individual item in a batch operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Success,
    Failed,
    Skipped,
    Cancelled,
}

/// Detailed outcome of processing a single batch item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemResult {
    pub id: usize,
    pub input: PathBuf,
    pub output: PathBuf,
    pub status: ItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_written: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Comprehensive summary of a completed batch job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub duration_ms: u64,
    pub items: Vec<BatchItemResult>,
}

impl BatchSummary {
    pub fn is_all_success(&self) -> bool {
        self.succeeded == self.total && self.total > 0
    }

    pub fn has_failures_or_cancellations(&self) -> bool {
        self.failed > 0 || self.cancelled > 0
    }
}

/// A planned item to process in a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItem {
    pub id: usize,
    pub input: PathBuf,
    pub output: PathBuf,
}

/// Progress notifications emitted during batch execution.
#[derive(Debug, Clone)]
pub enum BatchProgress<'a> {
    Started {
        total: usize,
    },
    ItemStarted {
        index: usize,
        total: usize,
        input: &'a Path,
        output: &'a Path,
    },
    ItemCompleted {
        index: usize,
        total: usize,
        result: &'a BatchItemResult,
    },
    Cancelled {
        completed: usize,
        remaining: usize,
    },
    Finished {
        summary: &'a BatchSummary,
    },
}

/// Resolves an output file path for an input file according to a naming pattern.
///
/// Supported template placeholders:
/// - `{stem}` or `{name}`: Input filename without extension
/// - `{ext}`: Target format extension (without dot)
/// - `{index}`: 1-based sequential index (1, 2, 3...)
/// - `{index0}`: 0-based sequential index (0, 1, 2...)
/// - `{index:02}`, `{index:03}`, `{index:04}`: Zero-padded 1-based index
/// - `{index0:02}`, `{index0:03}`, `{index0:04}`: Zero-padded 0-based index
pub fn resolve_output_path(
    input: &Path,
    output_dir: &Path,
    pattern: Option<&str>,
    target_ext: &str,
    index: usize,
) -> Result<PathBuf> {
    let clean_ext = target_ext.trim_start_matches('.');
    if clean_ext.is_empty() {
        return Err(FcError::InvalidOption(
            "target extension cannot be empty".to_string(),
        ));
    }

    let stem = input.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
        FcError::InvalidOption(format!("invalid input file path: {}", input.display()))
    })?;

    let raw_pattern = pattern.unwrap_or("{stem}.{ext}");

    // Replace placeholders
    let mut resolved = raw_pattern
        .replace("{stem}", stem)
        .replace("{name}", stem)
        .replace("{ext}", clean_ext)
        .replace("{index:04}", &format!("{:04}", index))
        .replace("{index:03}", &format!("{:03}", index))
        .replace("{index:02}", &format!("{:02}", index))
        .replace("{index}", &format!("{}", index))
        .replace("{index0:04}", &format!("{:04}", index.saturating_sub(1)))
        .replace("{index0:03}", &format!("{:03}", index.saturating_sub(1)))
        .replace("{index0:02}", &format!("{:02}", index.saturating_sub(1)))
        .replace("{index0}", &format!("{}", index.saturating_sub(1)));

    // Ensure extension exists if not provided in pattern
    let expected_suffix = format!(".{}", clean_ext);
    if !resolved.ends_with(&expected_suffix) && !raw_pattern.contains("{ext}") {
        resolved.push_str(&expected_suffix);
    }

    // Security & Path Safety checks
    if resolved.is_empty() {
        return Err(FcError::InvalidOption(
            "naming pattern resolved to an empty filename".to_string(),
        ));
    }
    if resolved.contains('/') || resolved.contains('\\') || resolved.contains("..") {
        return Err(FcError::InvalidOption(format!(
            "naming pattern resolved to unsafe path containing directory separators: '{}'",
            resolved
        )));
    }

    Ok(output_dir.join(resolved))
}

/// Prepares and validates a sequence of batch items, verifying that inputs are non-empty
/// and that no intra-batch output path collisions occur.
pub fn prepare_batch_items(
    inputs: &[PathBuf],
    output_dir: &Path,
    pattern: Option<&str>,
    target_ext: &str,
) -> Result<Vec<BatchItem>> {
    if inputs.is_empty() {
        return Err(FcError::InvalidOption(
            "batch requires at least one input file".to_string(),
        ));
    }

    let mut items = Vec::with_capacity(inputs.len());
    let mut seen_outputs = HashSet::new();

    for (idx, input) in inputs.iter().enumerate() {
        let item_id = idx + 1;
        let output = resolve_output_path(input, output_dir, pattern, target_ext, item_id)?;

        if !seen_outputs.insert(output.clone()) {
            return Err(FcError::InvalidOption(format!(
                "intra-batch destination collision detected: multiple inputs resolve to '{}'",
                output.display()
            )));
        }

        items.push(BatchItem {
            id: item_id,
            input: input.clone(),
            output,
        });
    }

    Ok(items)
}

/// Executes a sequential batch of items with independent failure isolation,
/// real-time progress callbacks, and cancellation checks.
///
/// If an item fails, the error is recorded for that item and execution proceeds
/// to subsequent items without aborting the entire batch.
///
/// If cancellation is triggered via `cancel_token`, the current item is allowed to finish
/// or safely roll back, remaining items are marked as cancelled, and a summary is returned.
pub fn execute_sequential_batch<F>(
    items: &[BatchItem],
    cancel_token: &CancellationToken,
    mut progress: impl FnMut(BatchProgress<'_>),
    mut worker: F,
) -> BatchSummary
where
    F: FnMut(&BatchItem) -> Result<u64>,
{
    let total = items.len();
    progress(BatchProgress::Started { total });

    let batch_start = Instant::now();
    let mut results = Vec::with_capacity(total);
    let mut succeeded = 0;
    let mut failed = 0;
    let skipped = 0;
    let mut cancelled = 0;

    for (idx, item) in items.iter().enumerate() {
        let item_index = idx + 1;

        if cancel_token.is_cancelled() {
            let remaining_count = total - idx;
            cancelled += remaining_count;

            for remaining_item in &items[idx..] {
                results.push(BatchItemResult {
                    id: remaining_item.id,
                    input: remaining_item.input.clone(),
                    output: remaining_item.output.clone(),
                    status: ItemStatus::Cancelled,
                    bytes_written: None,
                    error: Some("Operation cancelled by user".to_string()),
                    duration_ms: 0,
                });
            }

            progress(BatchProgress::Cancelled {
                completed: idx,
                remaining: remaining_count,
            });
            break;
        }

        progress(BatchProgress::ItemStarted {
            index: item_index,
            total,
            input: &item.input,
            output: &item.output,
        });

        let item_start = Instant::now();
        let res = worker(item);
        let duration_ms = item_start.elapsed().as_millis() as u64;

        match res {
            Ok(bytes) => {
                succeeded += 1;
                let item_res = BatchItemResult {
                    id: item.id,
                    input: item.input.clone(),
                    output: item.output.clone(),
                    status: ItemStatus::Success,
                    bytes_written: Some(bytes),
                    error: None,
                    duration_ms,
                };
                progress(BatchProgress::ItemCompleted {
                    index: item_index,
                    total,
                    result: &item_res,
                });
                results.push(item_res);
            }
            Err(err) => {
                let err_str = err.to_string();
                let status = if err_str.contains("cancelled") {
                    cancelled += 1;
                    ItemStatus::Cancelled
                } else {
                    failed += 1;
                    ItemStatus::Failed
                };

                let item_res = BatchItemResult {
                    id: item.id,
                    input: item.input.clone(),
                    output: item.output.clone(),
                    status,
                    bytes_written: None,
                    error: Some(err_str),
                    duration_ms,
                };
                progress(BatchProgress::ItemCompleted {
                    index: item_index,
                    total,
                    result: &item_res,
                });
                results.push(item_res);
            }
        }
    }

    let summary = BatchSummary {
        total,
        succeeded,
        failed,
        skipped,
        cancelled,
        duration_ms: batch_start.elapsed().as_millis() as u64,
        items: results,
    };

    progress(BatchProgress::Finished { summary: &summary });

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naming_pattern_resolution() {
        let out_dir = Path::new("target/batch_out");
        let input = Path::new("photos/vacation.jpeg");

        // Default pattern
        let p1 = resolve_output_path(input, out_dir, None, "png", 1).unwrap();
        assert_eq!(p1, out_dir.join("vacation.png"));

        // Stem and ext
        let p2 =
            resolve_output_path(input, out_dir, Some("{stem}-converted.{ext}"), "webp", 1).unwrap();
        assert_eq!(p2, out_dir.join("vacation-converted.webp"));

        // Index padding
        let p3 =
            resolve_output_path(input, out_dir, Some("{index:03}_{stem}.{ext}"), "png", 5).unwrap();
        assert_eq!(p3, out_dir.join("005_vacation.png"));

        // Automatic ext append if missing
        let p4 = resolve_output_path(input, out_dir, Some("{stem}_thumb"), "jpg", 1).unwrap();
        assert_eq!(p4, out_dir.join("vacation_thumb.jpg"));

        // 0-based index
        let p5 = resolve_output_path(input, out_dir, Some("{stem}_{index0}"), "png", 1).unwrap();
        assert_eq!(p5, out_dir.join("vacation_0.png"));
    }

    #[test]
    fn test_naming_pattern_rejects_unsafe_paths() {
        let out_dir = Path::new("target/batch_out");
        let input = Path::new("photos/vacation.jpeg");

        // Traversal
        let err =
            resolve_output_path(input, out_dir, Some("../escape.{ext}"), "png", 1).unwrap_err();
        assert!(matches!(err, FcError::InvalidOption(_)));

        // Slash in pattern
        let err =
            resolve_output_path(input, out_dir, Some("subdir/file.{ext}"), "png", 1).unwrap_err();
        assert!(matches!(err, FcError::InvalidOption(_)));
    }

    #[test]
    fn test_prepare_batch_items_collision_detection() {
        let out_dir = Path::new("target/batch_out");
        let inputs = vec![
            PathBuf::from("dir_a/test.png"),
            PathBuf::from("dir_b/test.png"),
        ];

        // Collides because both resolve to out/test.jpg
        let err = prepare_batch_items(&inputs, out_dir, None, "jpg").unwrap_err();
        assert!(
            err.to_string()
                .contains("intra-batch destination collision")
        );

        // Avoids collision when index is part of the pattern
        let items =
            prepare_batch_items(&inputs, out_dir, Some("{index}_{stem}.{ext}"), "jpg").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].output, out_dir.join("1_test.jpg"));
        assert_eq!(items[1].output, out_dir.join("2_test.jpg"));
    }

    #[test]
    fn test_sequential_batch_execution_with_independent_failures() {
        let items = vec![
            BatchItem {
                id: 1,
                input: PathBuf::from("good1.png"),
                output: PathBuf::from("good1.jpg"),
            },
            BatchItem {
                id: 2,
                input: PathBuf::from("bad.png"),
                output: PathBuf::from("bad.jpg"),
            },
            BatchItem {
                id: 3,
                input: PathBuf::from("good2.png"),
                output: PathBuf::from("good2.jpg"),
            },
        ];

        let cancel_token = CancellationToken::new();
        let mut progress_events = Vec::new();

        let summary = execute_sequential_batch(
            &items,
            &cancel_token,
            |ev| {
                if let BatchProgress::ItemCompleted { index, result, .. } = ev {
                    progress_events.push((index, result.status));
                }
            },
            |item| {
                if item.input.to_string_lossy().contains("bad") {
                    Err(FcError::Conversion("corrupt input image".to_string()))
                } else {
                    Ok(1024)
                }
            },
        );

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.cancelled, 0);
        assert!(!summary.is_all_success());
        assert!(summary.has_failures_or_cancellations());

        assert_eq!(progress_events.len(), 3);
        assert_eq!(progress_events[0], (1, ItemStatus::Success));
        assert_eq!(progress_events[1], (2, ItemStatus::Failed));
        assert_eq!(progress_events[2], (3, ItemStatus::Success));
    }

    #[test]
    fn test_sequential_batch_cancellation() {
        let items = vec![
            BatchItem {
                id: 1,
                input: PathBuf::from("1.png"),
                output: PathBuf::from("1.jpg"),
            },
            BatchItem {
                id: 2,
                input: PathBuf::from("2.png"),
                output: PathBuf::from("2.jpg"),
            },
            BatchItem {
                id: 3,
                input: PathBuf::from("3.png"),
                output: PathBuf::from("3.jpg"),
            },
        ];

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        let summary = execute_sequential_batch(
            &items,
            &cancel_token,
            |_| {},
            |item| {
                if item.id == 1 {
                    token_clone.cancel(); // Cancel after item 1 finishes
                    Ok(500)
                } else {
                    Ok(500)
                }
            },
        );

        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.cancelled, 2);
        assert_eq!(summary.items[1].status, ItemStatus::Cancelled);
        assert_eq!(summary.items[2].status, ItemStatus::Cancelled);
    }
}
