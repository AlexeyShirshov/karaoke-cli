//! Console status for long synchronous operations.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 3600 {
        format!(
            "{}h {:02}m {:02}s",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// Report liveness without pretending to know a completion percentage.
/// Dropping the guard wakes and joins the reporter, including on errors.
pub struct Activity {
    stop: Option<Sender<()>>,
    reporter: Option<JoinHandle<()>>,
}

impl Activity {
    pub fn start(label: impl Into<String>) -> Self {
        if !log::log_enabled!(log::Level::Info) {
            return Self {
                stop: None,
                reporter: None,
            };
        }
        Self::spawn(label.into(), Duration::from_secs(10))
    }

    fn spawn(label: String, interval: Duration) -> Self {
        let (stop, receiver) = mpsc::channel();
        let started = Instant::now();
        let reporter = thread::Builder::new()
            .name("console-status".into())
            .spawn(move || loop {
                match receiver.recv_timeout(interval) {
                    Err(RecvTimeoutError::Timeout) => log::info!(
                        "still working: {label} ({} elapsed)",
                        format_duration(started.elapsed())
                    ),
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                }
            });
        match reporter {
            Ok(reporter) => Self {
                stop: Some(stop),
                reporter: Some(reporter),
            },
            Err(error) => {
                log::warn!("cannot start console status reporter: {error}");
                Self {
                    stop: None,
                    reporter: None,
                }
            }
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(reporter) = self.reporter.take() {
            let _ = reporter.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_is_readable() {
        for (seconds, expected) in [
            (0, "0s"),
            (59, "59s"),
            (60, "1m 00s"),
            (139, "2m 19s"),
            (3600, "1h 00m 00s"),
            (3723, "1h 02m 03s"),
        ] {
            assert_eq!(format_duration(Duration::from_secs(seconds)), expected);
        }
    }

    #[test]
    fn stopping_reporter_does_not_wait_for_its_interval() {
        let activity = Activity::spawn("test activity".into(), Duration::from_secs(3600));
        assert!(activity.reporter.is_some());
        drop(activity);
    }
}
