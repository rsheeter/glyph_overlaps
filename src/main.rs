use std::path::{Path, PathBuf};

use fontdrasil::types::GlyphName;
use fontir::ir::GlyphPathBuilder;
use kurbo::{BezPath, Point, Rect, Shape};
use path_bool::path_from_path_data;


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
        return Self {
            name: glif.name().as_str().into(),
            source: file.to_path_buf(),
            bezpath: glif.contours.to_bezpath(glif.name().as_str()),
        }
    }

    fn from_glif_file(file: &Path) -> Self {        
        let glif = norad::Glyph::load(file).unwrap_or_else(|e| panic!("Unable to load {file:?}: {e}"));
        Self::from_glif(file, &glif)
    }

    fn from_ufo_file(ufo_dir: & Path) -> Vec<Self> {
        let ufo = norad::Font::load(ufo_dir).unwrap_or_else(|e| panic!("Error loading {ufo_dir:?}: {e}"));
        ufo.iter_layers()
            .flat_map(|l| l.iter().map(|g| (l.get_path(g.name()).unwrap_or_else(|| panic!("No path for {} in layer {}", g.name(), l.name())), g)))
            .map(|(glif_file, glif)| {
                
                Self::from_glif(glif_file, glif)
            })
            .collect()
    }

    fn has_fill_rule_discrepency(&self) -> bool {
        let bbox = self.bezpath.bounding_box();
        let bbox = Rect::new(bbox.min_x().floor(), bbox.min_y().floor(), bbox.max_x().ceil(), bbox.max_y().ceil());

        // Our svg is in font units. Just every whole font unit point in the bbox
        for x in bbox.min_x() as i32..bbox.max_x() as i32 {
            for y in bbox.min_y() as i32..bbox.max_y() as i32 {
                let winding = self.bezpath.winding(Point::new(x.into(), y.into()));
                let nonzero = winding != 0;
                let evenodd = winding % 2 == 1;
                if nonzero != evenodd {
                    return true;
                }
            }
        }
        false
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
            path_builder.end_path().unwrap_or_else(|e| panic!("Error making BezPath for {glyph_name}: {e}"));
        }
    
    
        path_builder
            .build()
            .unwrap_or_else(|e| panic!("Unable to create BezPath for {glyph_name}: {e}"))
    }
}

fn main() {
    eprintln!("WARNING: we're currently only checking simple glyphs, not components that transitively have problems");

    let glyphs = std::env::args().into_iter()
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
