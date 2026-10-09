use colored::Colorize;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};

pub fn get_progress_bar(content_len: u64, prefix: &str) -> ProgressBar {
    let pb = ProgressBar::new(content_len);

    pb.set_style(
        ProgressStyle::with_template(
            "{prefix} [{elapsed_precise}] [{wide_bar:.green/yellow}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .with_key(
            "eta",
            |state: &ProgressState, w: &mut dyn std::fmt::Write| {
                let _ = write!(w, "{:.1}s", state.eta().as_secs_f64());
            },
        )
        .progress_chars("#>-"),
    );

    pb.set_prefix(format!("{}", prefix.replace(".partial", "").yellow()));

    pb
}
