use std::path::{Path, PathBuf};

use fontdrasil::types::GlyphName;
use fontir::ir::GlyphPathBuilder;
use kurbo::{Affine, BezPath, Point, Rect, Shape};
use resvg::tiny_skia::{Pixmap, PremultipliedColorU8};

const _SAVE_DEBUG_IMAGES: bool = true;
const _OVERLAP_DETECTION: OverlapDetection = OverlapDetection::ImageDiff;

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

    fn create_svg(&self, fill_rule: resvg::usvg::FillRule) -> (Rect, String) {
        let mut svg = String::default();
        let bbox = self.bezpath.bounding_box();
        let margin = bbox.width().max(bbox.height()) * 0.1;
        let viewbox = Rect::new(
            (bbox.min_x() - margin).floor(),
            (bbox.min_y() - margin).floor(),
            (bbox.max_x() + margin).ceil(),
            (bbox.max_y() + margin).ceil(),
        );
        svg.push_str("<svg viewBox=\"");
        svg.push_str(&format!(
            "{} {} {} {}",
            viewbox.min_x(),
            viewbox.min_y(),
            viewbox.max_x(),
            viewbox.max_y()
        ));
        svg.push_str("\" xmlns=\"http://www.w3.org/2000/svg\">");
        svg.push_str("<path fill=\"gray\" fill-rule=\"");
        svg.push_str(match fill_rule {
            resvg::usvg::FillRule::EvenOdd => "evenodd",
            resvg::usvg::FillRule::NonZero => "nonzero",
        });
        svg.push_str("\" d=\"");
        svg.push_str(&self.bezpath.to_svg());
        svg.push_str("\"/></svg>");
        (viewbox, svg)
    }

    fn render_no_aa(&self, fill_rule: resvg::usvg::FillRule) -> Pixmap {
        let (viewbox, svg) = self.create_svg(fill_rule);
        let options: resvg::usvg::Options<'_> = resvg::usvg::Options {
            shape_rendering: resvg::usvg::ShapeRendering::OptimizeSpeed, // anti-aliasing off
            ..Default::default()
        };
        let tree = resvg::usvg::Tree::from_str(&svg, &options)
            .unwrap_or_else(|e| panic!("Unable to create Tree for {}: {e}", self.name));
        let mut pixmap = Pixmap::new(viewbox.width() as u32, viewbox.height() as u32)
            .unwrap_or_else(|| panic!("Unable to create pixmap"));
        resvg::render(&tree, Default::default(), &mut pixmap.as_mut());

        if _SAVE_DEBUG_IMAGES {
            let filename = format!(
                "/tmp/{}.{}.png",
                self.name,
                match fill_rule {
                    resvg::usvg::FillRule::EvenOdd => "evenodd",
                    resvg::usvg::FillRule::NonZero => "nonzero",
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

#[allow(unused)]
enum OverlapDetection {
    ComputeWindingALot,
    ImageDiff,
    FaceGraph,
}

impl OverlapDetection {
    fn has_fill_rule_discrepency(&self, glyph: &Glyph) -> bool {
        match self {
            OverlapDetection::ComputeWindingALot => Self::compute_winding_a_lot(glyph),
            OverlapDetection::ImageDiff => Self::image_diff(glyph),
            OverlapDetection::FaceGraph => todo!("Play with path-bool, DualGraph has the answer!"),
        }
    }

    /// Look for fill rule problems by plotting a lot of points winding
    ///
    /// Naively implemented, very slow, could be massively optimized. Don't bother, there are
    /// better approaches.
    fn compute_winding_a_lot(glyph: &Glyph) -> bool {
        let bbox = glyph.bezpath.bounding_box();
        let bbox = Rect::new(
            bbox.min_x().floor(),
            bbox.min_y().floor(),
            bbox.max_x().ceil(),
            bbox.max_y().ceil(),
        );

        // Our svg is in font units. Just every whole font unit point in the bbox
        for x in bbox.min_x() as i32..bbox.max_x() as i32 {
            for y in bbox.min_y() as i32..bbox.max_y() as i32 {
                let winding = glyph.bezpath.winding(Point::new(x.into(), y.into()));
                let nonzero = winding != 0;
                let evenodd = winding % 2 == 1;
                if nonzero != evenodd {
                    return true;
                }
            }
        }
        false
    }

    /// Look fill rule problems by rendering evenodd and nonzero and comparing.
    ///
    /// You'd think this woefully suboptimal but it gets you all the optimizations
    /// that have gone into rendering images for free so a naive implementation does OK.
    fn image_diff(glyph: &Glyph) -> bool {
        // render without AA, we just want insideness from the pixels
        let mut evenodd = glyph.render_no_aa(resvg::usvg::FillRule::EvenOdd);
        let nonzero = glyph.render_no_aa(resvg::usvg::FillRule::NonZero);

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
            let filename = format!("/tmp/{}.diff.png", glyph.name,);
            save_debug_image(&filename, &evenodd);
        }

        discrepency
    }
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
        if _OVERLAP_DETECTION.has_fill_rule_discrepency(glyph) {
            eprintln!("{:?} needs the overlap flag", glyph.source);
        }
    }
}
