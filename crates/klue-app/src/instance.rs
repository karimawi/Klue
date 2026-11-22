//! One Klue at a time. The first instance listens on a local socket (a named pipe on
//! Windows); a second launch connects to it, asks it to show its window, and exits.

use std::{
    io::{BufRead, BufReader, Write},
    sync::{Mutex, OnceLock},
};

use iced::futures::{SinkExt, Stream};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream as LocalStream, ToNsName, prelude::*};
use tokio::sync::mpsc;

const NAME: &str = "io.github.karimawi.klue.sock";

static REQUESTS: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<Request>>>> = OnceLock::new();

/// What a later launch asks the running Klue to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Request {
    Show,
    QuickView,
}

pub enum Start {
    /// This is the only Klue: carry on.
    First,
    /// Another Klue was running and has been asked to show itself.
    Second,
}

pub fn claim(quick_view: bool) -> Start {
    let Ok(name) = NAME.to_ns_name::<GenericNamespaced>() else { return Start::First };
    if let Ok(mut stream) = LocalStream::connect(name.clone()) {
        let _ = stream.write_all(if quick_view { b"quick-view\n" } else { b"show\n" });
        let _ = stream.flush();
        return Start::Second;
    }
    // Nobody answered, so a leftover socket file (after a crash) can be replaced.
    let listener = match ListenerOptions::new().name(name).try_overwrite(true).create_sync() {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("single-instance socket unavailable: {e}");
            return Start::First;
        }
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let _ = REQUESTS.set(Mutex::new(Some(rx)));
    std::thread::Builder::new()
        .name("klue-instance".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut line = String::new();
                let _ = BufReader::new(conn).read_line(&mut line);
                let request = if line.trim() == "quick-view" { Request::QuickView } else { Request::Show };
                if tx.send(request).is_err() {
                    break;
                }
            }
        })
        .ok();
    Start::First
}

/// Requests from later launches.
pub fn requests() -> impl Stream<Item = Request> {
    iced::stream::channel(4, async |mut output| {
        let Some(mut rx) = REQUESTS.get().and_then(|r| r.lock().ok()?.take()) else { return };
        while let Some(request) = rx.recv().await {
            if output.send(request).await.is_err() {
                break;
            }
        }
    })
}
