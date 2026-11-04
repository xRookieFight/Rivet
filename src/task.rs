//! Tiny background job helper.
//!
//! Every network or git call runs on its own thread and hands the result back
//! through a channel; the UI polls once per frame. Good enough for a desktop
//! client where requests are user driven and few at a time.

use egui::Context;
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub enum Load<T> {
    #[default]
    Idle,
    Loading(Receiver<Result<T, String>>),
    Ready(T),
    Failed(String),
}

impl<T: Send + 'static> Load<T> {
    /// Starts `job` in the background and repaints once it's done.
    pub fn spawn(ctx: &Context, job: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let res = job().map_err(|e| format!("{e:#}"));
            let _ = tx.send(res);
            ctx.request_repaint();
        });
        Load::Loading(rx)
    }

    /// Moves a finished result into place. Call it before reading the state.
    pub fn poll(&mut self) -> &mut Self {
        if let Load::Loading(rx) = self {
            match rx.try_recv() {
                Ok(Ok(v)) => *self = Load::Ready(v),
                Ok(Err(e)) => *self = Load::Failed(e),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => *self = Load::Failed("background task died".into()),
            }
        }
        self
    }

    pub fn ready(&mut self) -> Option<&mut T> {
        match self.poll() {
            Load::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_loading(&mut self) -> bool {
        matches!(self.poll(), Load::Loading(_))
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Load::Idle)
    }

    /// Takes a finished result out, leaving `Idle`. Handy for one shot actions
    /// like "star" or "merge" where we only care about the outcome once.
    pub fn take(&mut self) -> Option<Result<T, String>> {
        match self.poll() {
            Load::Ready(_) | Load::Failed(_) => match std::mem::take(self) {
                Load::Ready(v) => Some(Ok(v)),
                Load::Failed(e) => Some(Err(e)),
                _ => None,
            },
            _ => None,
        }
    }
}
