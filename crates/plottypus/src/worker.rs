use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use plottypus_core::{Error, Result, Snapshot};
use plottypus_metrics::Sampler;

const SLICE: Duration = Duration::from_millis(100);

pub enum Cmd {
    Interval(Duration),
    Paused(bool),
    Quit,
}

pub struct Handle {
    pub cmds: Sender<Cmd>,
    pub snaps: Receiver<Result<Snapshot>>,
    join: Option<JoinHandle<()>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.cmds.send(Cmd::Quit);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn spawn(interval: Duration) -> Result<Handle> {
    let (cmd_tx, cmd_rx) = channel();
    let (snap_tx, snap_rx) = channel();
    let builder = thread::Builder::new().name(String::from("plottypus-sampler"));
    let join = builder
        .spawn(move || run(cmd_rx, snap_tx, interval))
        .map_err(|err| Error::system(format!("spawn sampler thread: {err}")))?;
    Ok(Handle {
        cmds: cmd_tx,
        snaps: snap_rx,
        join: Some(join),
    })
}

#[allow(clippy::needless_pass_by_value)]
fn run(cmds: Receiver<Cmd>, snaps: Sender<Result<Snapshot>>, mut interval: Duration) {
    let mut sampler = match Sampler::new() {
        Ok(sampler) => sampler,
        Err(err) => {
            let _ = snaps.send(Err(err));
            return;
        }
    };
    let mut paused = false;
    let mut next = Instant::now();
    loop {
        let now = Instant::now();
        if now < next {
            let remaining = next - now;
            match cmds.recv_timeout(remaining.min(SLICE)) {
                Ok(Cmd::Interval(new)) => {
                    interval = new;
                    next = now + interval;
                }
                Ok(Cmd::Paused(new)) => paused = new,
                Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            continue;
        }
        if !paused && snaps.send(sampler.tick()).is_err() {
            break;
        }
        next = Instant::now() + interval;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn worker_streams_and_stops() {
        let handle = spawn(Duration::from_millis(20)).expect("spawn");
        let first = handle
            .snaps
            .recv_timeout(Duration::from_secs(2))
            .expect("snap1");
        assert!(first.is_ok());
        let second = handle
            .snaps
            .recv_timeout(Duration::from_secs(2))
            .expect("snap2");
        assert!(second.is_ok());

        handle.cmds.send(Cmd::Paused(true)).expect("send pause");
        handle
            .cmds
            .send(Cmd::Interval(Duration::from_millis(50)))
            .expect("send interval");
        handle.cmds.send(Cmd::Quit).expect("send quit");
        drop(handle);
    }

    #[test]
    fn dropping_handle_joins_worker() {
        let handle = spawn(Duration::from_millis(20)).expect("spawn");
        let _ = handle.snaps.recv_timeout(Duration::from_secs(2));
        drop(handle);
    }
}
