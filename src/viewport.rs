//! The browser window's size, for layout decisions CSS can't make (such as
//! how large the cards can be while all of them stay on screen).

use dioxus::prelude::*;

/// The window's inner size in rem. Zero until the first measurement.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

/// Starts tracking the window size and provides it as a `Signal<Viewport>`
/// context. Call once, from the root layout.
pub fn provide_viewport() {
    let mut viewport = use_context_provider(|| Signal::new(Viewport::default()));
    use_future(move || async move {
        let mut eval = document::eval(
            "const send = () => {
                const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
                dioxus.send([innerWidth / rem, innerHeight / rem]);
            };
            send();
            window.addEventListener('resize', send);",
        );
        while let Ok([width, height]) = eval.recv::<[f64; 2]>().await {
            let measured = Viewport { width, height };
            if *viewport.peek() != measured {
                viewport.set(measured);
            }
        }
    });
}
