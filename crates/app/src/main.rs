//! Leptos front end: runs the game loop, saves to localStorage, and renders. Game rules live in `sim`.
//!
//! Dev server: `trunk serve` from this directory.

use std::cell::Cell;
use std::time::Duration;

use leptos::prelude::*;
use sim::{Command, Game};

const SAVE_KEY: &str = "population-idle-save";
const SAVED_AT_KEY: &str = "population-idle-saved-at";
const FRAME_INTERVAL: Duration = Duration::from_millis(50);
const AUTOSAVE_MS: f64 = 10_000.0;
/// Offline progress is capped so a long absence can't freeze the page while it catches up.
const MAX_OFFLINE_MS: f64 = 8.0 * 60.0 * 60.0 * 1000.0;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let game = RwSignal::new(load_game());
    let snapshot = move || game.with(Game::snapshot);

    let last = Cell::new(now());
    let last_save = Cell::new(now());
    set_interval(
        move || {
            let t = now();
            // Only notify when the sim actually moved, so frames shorter than a tick cost nothing.
            let ticks = game.update_untracked(|g| g.advance(t - last.get()));
            if ticks > 0 {
                game.notify();
            }
            last.set(t);
            if t - last_save.get() > AUTOSAVE_MS {
                game.with_untracked(save_game);
                last_save.set(t);
            }
        },
        FRAME_INTERVAL,
    );
    let _ = window_event_listener(leptos::ev::beforeunload, move |_| game.with_untracked(save_game));

    let apply = move |command: Command| {
        game.update(|g| {
            let _ = g.apply(command);
        })
    };
    let reset = move |_| {
        clear_save();
        game.set(Game::new());
    };

    view! {
        <h1>"Population Idle"</h1>
        <p>
            {move || fmt(snapshot().points)} " points ("
            {move || fmt(snapshot().points_per_sec)} "/s)"
        </p>
        <p>
            <button on:click=move |_| apply(Command::Click)>"Click"</button>
            " "
            <button
                on:click=move |_| apply(Command::BuyGenerator)
                disabled=move || { let s = snapshot(); s.points < s.generator_cost }
            >
                "Buy generator (" {move || snapshot().generators} ") — "
                {move || fmt(snapshot().generator_cost)}
            </button>
        </p>
        <p>
            <button on:click=reset>"Reset save"</button>
        </p>
    }
}

fn fmt(n: f64) -> String {
    if n < 1000.0 { format!("{n:.1}") } else { format!("{n:.2e}") }
}

fn now() -> f64 {
    js_sys::Date::now()
}

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// Loads the saved game and fast-forwards it through the time spent away.
fn load_game() -> Game {
    let Some(storage) = storage() else { return Game::new() };
    let Some(json) = storage.get_item(SAVE_KEY).ok().flatten() else { return Game::new() };
    match Game::load(&json) {
        Ok(mut game) => {
            let saved_at = storage
                .get_item(SAVED_AT_KEY)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or_else(now);
            game.advance((now() - saved_at).clamp(0.0, MAX_OFFLINE_MS));
            game
        }
        Err(e) => {
            leptos::logging::warn!("Could not load save, starting fresh: {e}");
            Game::new()
        }
    }
}

fn save_game(game: &Game) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(SAVE_KEY, &game.save());
        let _ = storage.set_item(SAVED_AT_KEY, &now().to_string());
    }
}

fn clear_save() {
    if let Some(storage) = storage() {
        let _ = storage.remove_item(SAVE_KEY);
        let _ = storage.remove_item(SAVED_AT_KEY);
    }
}
