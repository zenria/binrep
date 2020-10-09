use crate::progress::non_interactive::NonInteractiveProgress;
use crate::progress::{Progress, ProgressReporter};
use atty::Stream;
use indicatif::{ProgressBar, ProgressStyle};

/// Reporter that either display a nice progress bar or ticker on interactive
/// cli session, or use the non interactive reporter when on a non interactive session
pub struct IndicatifProgressReporter;

impl ProgressReporter for IndicatifProgressReporter {
    type Output = IndicatifProgress;

    fn create(name: Option<String>, max: Option<usize>) -> Self::Output {
        let pb = max
            .map(|length| ProgressBar::new(length as u64))
            .unwrap_or(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes:>7}/{total_bytes:7} {msg}")
                .progress_chars("##-"),
        );
        if let Some(name) = name {
            pb.set_message(&name);
        }
        IndicatifProgress(pb)
    }
}

pub struct IndicatifProgress(ProgressBar);

impl Progress for IndicatifProgress {
    fn inc(&mut self, amount: usize) {
        self.0.inc(amount as u64)
    }

    fn tick(&mut self) {
        self.0.tick()
    }
}
