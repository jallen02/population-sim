//! Leptos front end: runs the game loop, saves to localStorage, and renders. Game rules live in `sim`.
//!
//! Dev server: `trunk serve` from this directory.

use std::cell::Cell;
use std::time::Duration;

use leptos::html::Canvas;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use sim::Game;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const SAVE_KEY: &str = "population-idle-save";
const SAVED_AT_KEY: &str = "population-idle-saved-at";
const FRAME_INTERVAL: Duration = Duration::from_millis(50);
const AUTOSAVE_MS: f64 = 10_000.0;
/// The pyramid's age axis always covers at least this many years, so its shape stays stable.
const PYRAMID_MIN_AGES: usize = 100;
/// Years between age labels on the pyramid.
const PYRAMID_LABEL_EVERY: usize = 10;
const PYRAMID_LABEL_WIDTH: f64 = 28.0;
const PYRAMID_BAR_COLOR: &str = "#4a8";
const PYRAMID_LABEL_COLOR: &str = "#888";
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

    let canvas = NodeRef::<Canvas>::new();
    Effect::new(move |_| {
        let ages = snapshot().ages;
        if let Some(canvas) = canvas.get() {
            draw_pyramid(&canvas, &ages);
        }
    });

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
    let _ = window_event_listener(leptos::ev::beforeunload, move |_| {
        game.with_untracked(save_game)
    });

    let reset = move |_| {
        clear_save();
        game.set(Game::new());
    };

    view! {
        <h1>"Population Idle"</h1>
        <h2>"Population: " {move || snapshot().population}</h2>
        <p>
            "Food: " {move || format!("{:.0}", snapshot().food)}
            " (+" {move || format!("{:.0}", snapshot().food_production)} "/yr, −"
            {move || format!("{:.0}", snapshot().food_consumption)} "/yr)"
        </p>
        <canvas class="pyramid" node_ref=canvas></canvas>
        <p>
            <button on:click=reset>"Reset save"</button>
        </p>
    }
}

/// Draws one centered bar per age, youngest at the top, sized to the canvas's CSS box.
fn draw_pyramid(canvas: &HtmlCanvasElement, ages: &[u64]) {
    let Some(ctx) = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|ctx| ctx.dyn_into::<CanvasRenderingContext2d>().ok())
    else {
        return;
    };
    // Match the backing store to the displayed size so bars stay crisp on high-DPI screens.
    // Resizing also clears the canvas.
    let dpr = window().device_pixel_ratio();
    let (width, height) = (canvas.client_width() as f64, canvas.client_height() as f64);
    canvas.set_width((width * dpr) as u32);
    canvas.set_height((height * dpr) as u32);
    let _ = ctx.scale(dpr, dpr);

    let rows = ages.len().max(PYRAMID_MIN_AGES);
    let row_height = height / rows as f64;
    let half_width = (width - PYRAMID_LABEL_WIDTH) / 2.0;
    let center = PYRAMID_LABEL_WIDTH + half_width;
    let max = ages.iter().copied().max().unwrap_or(0).max(1) as f64;

    ctx.set_fill_style_str(PYRAMID_BAR_COLOR);
    for (age, &count) in ages.iter().enumerate() {
        let bar = count as f64 / max * half_width;
        let top = age as f64 * row_height;
        ctx.fill_rect(center - bar, top, bar * 2.0, row_height);
    }

    ctx.set_fill_style_str(PYRAMID_LABEL_COLOR);
    ctx.set_font("10px system-ui, sans-serif");
    ctx.set_text_align("right");
    ctx.set_text_baseline("middle");
    for age in (0..rows).step_by(PYRAMID_LABEL_EVERY) {
        let middle = (age as f64 + 0.5) * row_height;
        let _ = ctx.fill_text(&age.to_string(), PYRAMID_LABEL_WIDTH - 4.0, middle);
    }
}

fn now() -> f64 {
    js_sys::Date::now()
}

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// Loads the saved game and fast-forwards it through the time spent away.
fn load_game() -> Game {
    let Some(storage) = storage() else {
        return Game::new();
    };
    let Some(json) = storage.get_item(SAVE_KEY).ok().flatten() else {
        return Game::new();
    };
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
