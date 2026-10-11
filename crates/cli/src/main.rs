//! Headless runner for balancing and testing: runs the sim and prints snapshots.
//!
//! Usage: cargo run -p cli -- [seconds] [print_every_seconds]

use sim::{Game, Snapshot, TICK_MS};

fn main() {
    let mut args = std::env::args()
        .skip(1)
        .map(|a| a.parse::<u64>().expect("expected a number"));
    let seconds = args.next().unwrap_or(600);
    let print_every = args.next().unwrap_or(60);
    let ticks_per_sec = (1000.0 / TICK_MS) as u64;

    let mut game = Game::new();
    print_row(&game.snapshot());
    for _ in 0..seconds * ticks_per_sec {
        game.tick();
        let snapshot = game.snapshot();
        if snapshot.tick.is_multiple_of(print_every * ticks_per_sec) {
            print_row(&snapshot);
        }
    }
}

fn print_row(snapshot: &Snapshot) {
    println!("{snapshot:?}");
}
