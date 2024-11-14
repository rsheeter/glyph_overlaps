use std::path::{Path, PathBuf};

use fontdrasil::types::GlyphName;
use fontir::ir::GlyphPathBuilder;
use kurbo::{Affine, BezPath, Rect, Shape};
use tiny_skia::FillRule;
use tiny_skia::{Paint, Pixmap, PremultipliedColorU8};

const _SAVE_DEBUG_IMAGES: bool = true;

#[derive(Debug)]
struct Glyph {
    name: GlyphName,
    source: PathBuf,
    bezpath: BezPath,
}

impl Glyph {
    pub fn from_file(file: impl AsRef<Path>) -> Vec<Self> {
        let file = file.as_ref();
        match file.extension().and_then(|e| e.to_str()) {
            Some("designspace") => todo!("load designspace"),
            Some("ufo") => Self::from_ufo_file(file),
            Some("glif") => vec![Self::from_glif_file(file)],
            _ => panic!("No handler for {file:?}"),
        }
    }

    fn from_glif(file: &Path, glif: &norad::Glyph) -> Self {
        // Font units and svg units don't agree on y-up.
        // It's very disconcerting to see all the glyphs upside down in test renders
        let mut bezpath = glif.contours.to_bezpath(glif.name().as_str());
        bezpath.apply_affine(Affine::FLIP_Y);
        return Self {
            name: glif.name().as_str().into(),
            source: file.to_path_buf(),
            bezpath,
        };
    }

    fn from_glif_file(file: &Path) -> Self {
        let glif =
            norad::Glyph::load(file).unwrap_or_else(|e| panic!("Unable to load {file:?}: {e}"));
        Self::from_glif(file, &glif)
    }

    fn from_ufo_file(ufo_dir: &Path) -> Vec<Self> {
        let ufo =
            norad::Font::load(ufo_dir).unwrap_or_else(|e| panic!("Error loading {ufo_dir:?}: {e}"));
        ufo.iter_layers()
            .flat_map(|l| {
                l.iter().map(|g| {
                    (
                        l.get_path(g.name()).unwrap_or_else(|| {
                            panic!("No path for {} in layer {}", g.name(), l.name())
                        }),
                        g,
                    )
                })
            })
            .map(|(glif_file, glif)| Self::from_glif(glif_file, glif))
            .collect()
    }

    /// Look fill rule problems by rendering evenodd and nonzero and comparing.
    ///
    /// You'd think this woefully suboptimal but it gets you all the optimizations
    /// that have gone into rendering images for free so a naive implementation does OK.
    fn has_fill_rule_discrepency(self: &Glyph) -> bool {
        // render without AA, we just want insideness from the pixels
        let mut evenodd = self.render_no_aa(FillRule::EvenOdd);
        let nonzero = self.render_no_aa(FillRule::Winding);

        if evenodd.pixels().len() != nonzero.pixels().len() {
            panic!("Inconsistent pixel count, seems very bad")
        }

        let pink = PremultipliedColorU8::from_rgba(255, 20, 147, 255).unwrap();
        let mut discrepency = false;
        for (evenodd_px, _) in evenodd
            .pixels_mut()
            .iter_mut()
            .zip(nonzero.pixels().iter())
            .filter(|(a, b)| a != b)
        {
            discrepency = true;
            *evenodd_px = pink;
        }

        if _SAVE_DEBUG_IMAGES {
            let filename = format!("/tmp/{}.diff.png", self.name,);
            save_debug_image(&filename, &evenodd);
        }

        discrepency
    }

    fn create_path(&self) -> (Rect, tiny_skia::Path) {
        // move the path to start at 0,0
        let mut bez = self.bezpath.clone();
        let bbox = self.bezpath.bounding_box();
        let margin = bbox.width().max(bbox.height()) * 0.1;
        bez.apply_affine(Affine::translate((
            -bbox.min_x() + margin,
            -bbox.min_y() + margin,
        )));
        let bbox = bez.bounding_box(); // bbox just changed
        let width = bbox.max_x() + margin;
        let height = bbox.max_y() + margin;

        let mut pb = tiny_skia::PathBuilder::new();
        for el in bez.iter() {
            match el {
                kurbo::PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
                kurbo::PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
                kurbo::PathEl::QuadTo(c, p) => {
                    pb.quad_to(c.x as f32, c.y as f32, p.x as f32, p.y as f32)
                }
                kurbo::PathEl::CurveTo(c0, c1, p) => pb.cubic_to(
                    c0.x as f32,
                    c0.y as f32,
                    c1.x as f32,
                    c1.y as f32,
                    p.x as f32,
                    p.y as f32,
                ),
                kurbo::PathEl::ClosePath => pb.close(),
            }
        }

        (
            Rect::new(0.0, 0.0, width, height),
            pb.finish()
                .unwrap_or_else(|| panic!("Unable to create path for {}", self.name)),
        )
    }

    fn render_no_aa(&self, fill_rule: FillRule) -> Pixmap {
        let (extents, path) = self.create_path();
        let mut pixmap = Pixmap::new(extents.width() as u32, extents.height() as u32)
            .unwrap_or_else(|| panic!("Unable to create pixmap"));
        let mut paint = Paint::default();
        paint.set_color_rgba8(128, 128, 128, 255); // gray
        paint.anti_alias = false; // just confuses diffs
        pixmap.fill_path(
            &path,
            &paint,
            fill_rule,
            tiny_skia::Transform::identity(),
            None,
        );

        if _SAVE_DEBUG_IMAGES {
            let filename = format!(
                "/tmp/{}.{}.png",
                self.name,
                match fill_rule {
                    FillRule::EvenOdd => "evenodd",
                    FillRule::Winding => "nonzero",
                }
            );
            save_debug_image(&filename, &pixmap);
        }
        pixmap
    }
}

fn save_debug_image(filename: &str, pixmap: &Pixmap) {
    std::fs::write(
        filename,
        pixmap
            .encode_png()
            .unwrap_or_else(|e| panic!("Failed to encode png for {filename}: {e}")),
    )
    .unwrap_or_else(|e| panic!("Failed to write {filename}: {e}"));
    eprintln!("Wrote {filename}");
}

trait ToBezPath {
    fn to_bezpath(&self, glyph_name: &str) -> BezPath;
}

impl ToBezPath for [norad::Contour] {
    /// Basically copied from <https://github.com/googlefonts/fontc/blob/9b7a5634dc0487d52af7a1528520306fc2c6941b/ufo2fontir/src/toir.rs#L31C1-L59C2>
    fn to_bezpath(&self, glyph_name: &str) -> BezPath {
        let mut path_builder = GlyphPathBuilder::new(glyph_name.into(), 32);

        for contour in self {
            for node in contour.points.iter() {
                match node.typ {
                    norad::PointType::Move => path_builder.move_to((node.x, node.y)),
                    norad::PointType::Line => path_builder.line_to((node.x, node.y)),
                    norad::PointType::QCurve => path_builder.qcurve_to((node.x, node.y)),
                    norad::PointType::Curve => path_builder.curve_to((node.x, node.y)),
                    norad::PointType::OffCurve => path_builder.offcurve((node.x, node.y)),
                }
                .unwrap_or_else(|e| panic!("Error making BezPath for {glyph_name}: {e}"));
            }
            path_builder
                .end_path()
                .unwrap_or_else(|e| panic!("Error making BezPath for {glyph_name}: {e}"));
        }

        path_builder
            .build()
            .unwrap_or_else(|e| panic!("Unable to create BezPath for {glyph_name}: {e}"))
    }
}

fn main() {
    eprintln!("WARNING: we're currently only checking simple glyphs, not components that transitively have problems");

    let glyphs = std::env::args()
        .into_iter()
        .skip(1)
        .flat_map(|a| Glyph::from_file(&a).into_iter())
        .collect::<Vec<_>>();

    eprintln!("Loaded {}", glyphs.len());

    for glyph in &glyphs {
        if glyph.has_fill_rule_discrepency() {
            eprintln!("{:?} needs the overlap flag", glyph.source);
        }
    }
}
