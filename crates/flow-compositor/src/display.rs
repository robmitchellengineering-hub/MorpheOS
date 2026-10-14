//! Which DRM card shows the shell (`--drm auto`), as pure logic.
//!
//! WHY NOT `/dev/dri/card0`. On a laptop whose kernel has `simpledrm` built in, the
//! firmware's framebuffer becomes `card0` at boot. The real driver (`amdgpu`, `i915`, `xe`)
//! then loads, takes the display over from it, and registers as `card1`: card numbers are
//! not reused. A shell told to use `card0` fails there on a machine whose graphics work, and
//! on a machine with two GPUs the panel may hang off either. So the shell tries every card,
//! in the kernel's order, and takes the first that has a connected display with modes.
//!
//! The order and the choice live here, where they are tested on any machine (AGENTS.md H9,
//! H16); `live.rs` supplies the attempt, which opens the card through the seat and sets a
//! mode on it.

/// The card nodes among `names` (the entries of `/dev/dri`), in the kernel's order:
/// `card0`, `card1`, …, `card10`. Render nodes (`renderD128`) are not displays.
pub fn card_order<I, S>(names: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut cards: Vec<(u32, String)> = names
        .into_iter()
        .map(Into::into)
        .filter_map(|n| {
            let num = n.strip_prefix("card")?.parse::<u32>().ok()?;
            Some((num, n))
        })
        .collect();
    cards.sort();
    cards.into_iter().map(|(_, n)| n).collect()
}

/// Try `attempt` on each card in turn and keep the first that succeeds, with its name. If
/// none does, the error names every card and why it was passed over, because "no display"
/// alone is no help to a person reading the log of a black screen.
pub fn choose<T>(
    cards: &[String],
    mut attempt: impl FnMut(&str) -> Result<T, String>,
) -> Result<(String, T), String> {
    if cards.is_empty() {
        return Err("no DRM card (nothing in /dev/dri is a card)".into());
    }
    let mut passed = Vec::new();
    for card in cards {
        match attempt(card) {
            Ok(t) => return Ok((card.clone(), t)),
            Err(why) => passed.push(format!("{card}: {why}")),
        }
    }
    Err(format!(
        "no card can show the shell ({})",
        passed.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_come_in_the_kernels_order_and_render_nodes_are_left_out() {
        let names = ["renderD128", "card10", "card1", "by-path", "card0", "card2"];
        assert_eq!(card_order(names), ["card0", "card1", "card2", "card10"]);
    }

    #[test]
    fn names_that_only_look_like_cards_are_left_out() {
        assert!(card_order(["card", "cardX", "card0-eDP-1", "card-1"]).is_empty());
    }

    #[test]
    fn the_first_card_that_shows_the_shell_wins() {
        // The laptop case: simpledrm's card0 is gone (taken over), the real driver is card1.
        let cards = card_order(["card0", "card1", "card2"]);
        let mut tried = Vec::new();
        let chosen = choose(&cards, |c| {
            tried.push(c.to_string());
            match c {
                "card0" => Err("no connected display".into()),
                "card1" => Ok(1920),
                _ => Ok(1280),
            }
        });
        assert_eq!(chosen, Ok(("card1".to_string(), 1920)));
        assert_eq!(
            tried,
            ["card0", "card1"],
            "it stops at the first that works"
        );
    }

    #[test]
    fn card0_is_still_taken_when_it_works() {
        let cards = card_order(["card0", "card1"]);
        assert_eq!(choose(&cards, |_| Ok(())), Ok(("card0".to_string(), ())));
    }

    #[test]
    fn when_no_card_works_the_error_says_why_for_each() {
        let cards = card_order(["card0", "card1"]);
        let err = choose::<()>(&cards, |c| Err(format!("{c} said no"))).unwrap_err();
        assert!(err.contains("card0: card0 said no"), "{err}");
        assert!(err.contains("card1: card1 said no"), "{err}");
    }

    #[test]
    fn no_cards_at_all_is_an_error_not_a_panic() {
        let err = choose::<()>(&[], |_| Ok(())).unwrap_err();
        assert!(err.contains("no DRM card"), "{err}");
    }
}
