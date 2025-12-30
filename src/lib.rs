use std::{io::Stdout, thread};

use ratatui::{Terminal, prelude::CrosstermBackend};
use tokio::{runtime, sync::mpsc};

use crate::app::{App, task::TaskManager};

pub mod app;
pub mod window;

pub fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    let runtime = runtime::Builder::new_multi_thread().enable_all().build()?;
    let (tx, rx) = mpsc::channel(32);
    let background = thread::spawn(move || {
        let mut manager = TaskManager::new(runtime, rx);
        manager.run();
    });
    let app = App::new(tx);
    app.run(terminal)?;
    background.join().unwrap();
    Ok(())
}
