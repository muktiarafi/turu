//! Turu: a thread that waits for the GPU to finish given submissions and then runs what was
//! asked for, in order. The shell gives a client's surfaces back by it while its main thread
//! waits for the display's refresh: polled from there, a surface came back about 4 refreshes
//! after it went (docs/STATUS.md, "Pictures under way together").
//!
//! The GPU finishes submissions in order, so waiting for each in turn takes no longer than the
//! work itself. A wait holds only what a submit shares with it (wgpu-core's snatch lock, read),
//! so the threads that submit don't wait for it.

use std::sync::mpsc;
use std::time::Duration;

/// The longest a wait lasts (a lost device, a hung driver): what was asked for runs then anyway.
const WAIT_AT_MOST: Duration = Duration::from_millis(500);

type Job = (wgpu::SubmissionIndex, Box<dyn FnOnce() + Send>);

pub struct GpuWaiter {
    jobs: mpsc::Sender<Job>,
}

impl GpuWaiter {
    /// Starts the thread (`name` in debuggers and crash reports) for `device`; it ends with the
    /// last handle to this.
    pub fn new(device: wgpu::Device, name: &str) -> std::io::Result<Self> {
        let (jobs, waiting) = mpsc::channel::<Job>();
        std::thread::Builder::new().name(name.into()).spawn(move || {
            for (index, then) in waiting {
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: Some(WAIT_AT_MOST),
                });
                then();
            }
        })?;
        Ok(Self { jobs })
    }

    /// Runs `then` on the waiter's thread once the GPU has finished submission `index` (and
    /// everything before it).
    pub fn after(&self, index: wgpu::SubmissionIndex, then: impl FnOnce() + Send + 'static) {
        let _ = self.jobs.send((index, Box::new(then)));
    }
}
