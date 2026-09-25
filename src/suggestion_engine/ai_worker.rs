//! Runs slow palette work (AI requests, network sends) off the input thread so
//! the terminal stays responsive and the user can cancel with Esc.

use std::sync::mpsc::Sender;
use std::thread;

/// Run `job` on a new thread and deliver `wrap(id, result)` through `tx`.
/// A cancelled job still finishes, but its result is ignored by the receiver.
pub fn spawn_job<T, M>(
    id: u64,
    tx: Sender<M>,
    job: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    wrap: fn(u64, anyhow::Result<T>) -> M,
) where
    T: Send + 'static,
    M: Send + 'static,
{
    thread::spawn(move || {
        let result = job();
        let _ = tx.send(wrap(id, result));
    });
}
