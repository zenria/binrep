use crate::progress::indicatif::{IndicatifProgress, IndicatifProgressReporter};
use crate::progress::non_interactive::{NonInteractiveProgress, NonInteractiveProgressReporter};
use crate::progress::{Progress, ProgressReporter};
use atty::Stream;
use indicatif::{ProgressBar, ProgressStyle};

/// Reporter that either display a nice progress bar or ticker on interactive
/// cli session, or use the non interactive reporter when on a non interactive session
pub struct InteractiveProgressReporter;

impl ProgressReporter for InteractiveProgressReporter {
    type Output = InteractiveProgress;

    fn create(name: Option<String>, max: Option<usize>) -> Self::Output {
        if atty::isnt(Stream::Stderr) {
            InteractiveProgress::NonInteractive(NonInteractiveProgressReporter::create(name, max))
        } else {
            InteractiveProgress::Interactive(IndicatifProgressReporter::create(name, max))
        }
    }
}

pub enum InteractiveProgress {
    Interactive(IndicatifProgress),
    NonInteractive(NonInteractiveProgress),
}

impl Progress for InteractiveProgress {
    fn inc(&mut self, amount: usize) {
        match self {
            InteractiveProgress::Interactive(p) => p.inc(amount),
            InteractiveProgress::NonInteractive(p) => p.inc(amount),
        }
    }

    fn tick(&mut self) {
        match self {
            InteractiveProgress::Interactive(p) => p.tick(),
            InteractiveProgress::NonInteractive(p) => p.tick(),
        }
    }
}
