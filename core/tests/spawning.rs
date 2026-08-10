//! Spawn-model distribution checks over a seeded run (loose bounds - the
//! point is the shape of the distribution, not exact frequencies).

use elevato_core::challenge::{Challenge, Condition};
use elevato_core::controller::Controller;
use elevato_core::{World, headless};

/// Watches without commanding. Nobody is ever delivered (the parked
/// elevator collects floor-0 spawns via re-arrivals but never moves), so
/// the final passenger list is the complete spawn census.
struct Bystander;

impl Controller for Bystander {
    fn init(&mut self, _world: &mut World) {}
}

#[test]
fn spawn_distribution_matches_the_original_frequencies() {
    let challenge = Challenge::new(8, 1, vec![4], 5.0, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 12345);
    headless::run(&mut world, &mut Bystander, 6000, 1);

    let passengers = world.passengers();
    let total = passengers.len();
    assert!(total > 450, "expected ~500 spawns in 100 s, got {total}");

    for passenger in passengers {
        assert!((55..=100).contains(&passenger.weight()));
        assert_ne!(passenger.current_floor(), passenger.destination_floor());
        assert!(passenger.destination_floor() < 8);
    }

    // Ground-floor spawn probability is 0.5 + 0.5/floor_count = 0.5625.
    let ground = passengers
        .iter()
        .filter(|passenger| passenger.current_floor() == 0)
        .count();
    let ground_share = ground as f64 / total as f64;
    assert!(
        (0.48..=0.65).contains(&ground_share),
        "ground-floor spawn share {ground_share:.3}, expected ≈ 0.56"
    );

    // ~91% (10/11) of upper-floor spawns head for the lobby.
    let upper: Vec<_> = passengers
        .iter()
        .filter(|passenger| passenger.current_floor() != 0)
        .collect();
    let to_lobby = upper
        .iter()
        .filter(|passenger| passenger.destination_floor() == 0)
        .count();
    let lobby_share = to_lobby as f64 / upper.len() as f64;
    assert!(
        (0.85..=0.96).contains(&lobby_share),
        "lobby-bound share of upper spawns {lobby_share:.3}, expected ≈ 0.91"
    );
}
