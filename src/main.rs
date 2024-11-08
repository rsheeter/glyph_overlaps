
use norad::Glyph;

fn main() {
    let glyph_files = std::env::args().into_iter()
        .skip(1)
        .map(|a| Glyph::load(&a).unwrap_or_else(|e| panic!("Unable to load {a}: {e}")))

        .collect::<Vec<_>>();

    eprintln!("Loaded {glyph_files:?}");
}
