//! Headless runner for balancing and testing: plays the sim with a bot and prints snapshots.
//!
//! Usage: cargo run -p cli -- [seconds] [print_every_seconds]

use sim::{Command, Game, Snapshot, TICK_MS};

fn main() {
    let mut args = std::env::args().skip(1).map(|a| a.parse::<u64>().expect("expected a number"));
    let seconds = args.next().unwrap_or(600);
    let print_every = args.next().unwrap_or(60);
    let ticks_per_sec = (1000.0 / TICK_MS) as u64;

    let mut game = Game::new();
    print_row(&game.snapshot());
    for _ in 0..seconds * ticks_per_sec {
        play(&mut game);
        game.tick();
        let snapshot = game.snapshot();
        if snapshot.tick.is_multiple_of(print_every * ticks_per_sec) {
            print_row(&snapshot);
        }
    }
}

/// The bot: replace with whatever strategy you want to measure.
fn play(game: &mut Game) {
    if game.state.tick.is_multiple_of(2) {
        let _ = game.apply(Command::Click);
    }
    while game.apply(Command::BuyGenerator).is_ok() {}
}

fn print_row(snapshot: &Snapshot) {
    println!("{snapshot:?}");
}
