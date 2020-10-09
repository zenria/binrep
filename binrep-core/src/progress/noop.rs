use crate::progress::{Progress, ProgressReporter};

pub struct NOOPProgress;

impl ProgressReporter for NOOPProgress {
    type Output = NOOPProgress;

    fn create(name: Option<String>, max: Option<usize>) -> NOOPProgress {
        NOOPProgress
    }
}

impl Progress for NOOPProgress {
    fn inc(&mut self, amount: usize) {
        // do nothing
    }

    fn tick(&mut self) {
        // do nothing
    }
}
