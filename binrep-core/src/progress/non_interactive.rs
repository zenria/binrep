use crate::progress::{Progress, ProgressReporter};
use indicatif::HumanBytes;

/// Progress reporter suitable for non interactive programs
/// outputs progress to stderr
///
pub struct NonInteractiveProgressReporter;

impl ProgressReporter for NonInteractiveProgressReporter {
    type Output = NonInteractiveProgress;

    fn create(name: Option<String>, max: Option<usize>) -> Self::Output {
        if let Some(name) = name {
            println!("{}", name);
        }

        NonInteractiveProgress { max, done: 0 }
    }
}

pub struct NonInteractiveProgress {
    max: Option<usize>,
    done: usize,
}

impl Progress for NonInteractiveProgress {
    fn inc(&mut self, amount: usize) {
        if let Some(max) = &self.max {
            let cur_pc = 100 * self.done / max;
            self.done += amount;
            let next_pc = 100 * self.done / max;
            if cur_pc != next_pc {
                eprintln!(
                    " {} .......... .......... .......... .......... .......... {}%",
                    HumanBytes(self.done as u64),
                    next_pc
                )
            }
        }
    }

    fn tick(&mut self) {
        // does nothing;
    }
}
