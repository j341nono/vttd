mod app;
mod commands;
mod config;
mod db;
mod error;
mod export;
mod filters;
mod input;
mod models;
mod pomodoro;
mod ui;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::config::{load_or_init, Paths};
use crate::db::Db;

const TICK: Duration = Duration::from_millis(250);

fn main() -> Result<()> {
    let paths = Paths::resolve()?;
    paths.ensure_dirs()?;
    let config = load_or_init(&paths)?;

    let db = Db::open(&paths.db_file)?;
    db.ensure_projects(&config.projects.default)?;
    db.seed_if_empty()?;

    let app = App::new(db, config, paths)?;
    run(app)
}

fn run(mut app: App) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut app, &mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn run_loop(app: &mut App, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = TICK
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Err(e) = app.on_key(key) {
                        app.status_message = Some(format!("err: {e}"));
                    }
                }
            }
        }

        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            // Currently nothing to tick beyond keeping the pomodoro clock fresh.
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
