# glyph_overlaps

For context, setting the glyf overlap flag is needed for OSX to avoid "holes" appearing
when rendering outlines with overlaps due to use of `evenodd`. Variable fonts are wont
to use overlaps, prior to variable fonts they were relatively rare.

For example, the [A from Roboto Flex](https://github.com/googlefonts/roboto-flex/blob/main/sources/1A-drawings/Mains/RobotoFlex_wght400.ufo/glyphs/A_.glif) has overlaps:

| nonzero | evenodd | diff highlighted |
| ------------- | ------------- | ------------- |
| ![nonzero](resources/images/A.nonzero.png) | ![evenodd](resources/images/A.evenodd.png) | ![diff](resources/images/A.diff.png) |

Usage

```shell
# General
$ cargo run -- path/to/file.glif
$ cargo run -- path/to/dir.ufo
$ cargo run -- path/to/file.designspace

# Assuming a sibling clone of https://github.com/googlefonts/roboto-flex
$ cargo run -- ../roboto-flex/sources/1A-drawings/Mains/RobotoFlex_wght400.ufo/glyphs/A_.glif
```

## Potential approaches

1. Have humans inspect the outlines and maintain a list of glyphs that need the bit set
   * We are actually doing this for some fonts. Sounds tiresome.
1. Render nonzero and evenodd, if they don't match then we need the overlap bit
   * This actually seems to work, albeit potentially at cost of doing extra work
   * `overlap.py` does this, it was the original idea: just directly check
   * Might miss very small overlaps but that's likely OK for our use case
   * Ideally rendering would be done w/o anti-aliasing, we just want the insideness signal
       * Since these days resvg is under linebender maybe we can support that?
1. Rendering images seems slow and indirect, why not just check directly?
   * Brute force: compute winding for a whole bunch of points (say the upem grid) and see if any of them would have different results based on fill rule
      * This is very slow when done naively, rasterizers have all sorts of optimizations
      * We could start optimizing but the ROI seems low
   * @raphlinus suggested https://github.com/GraphiteEditor/Graphite/tree/master/libraries/path-bool likely has all the parts needed to detect differences in winding
      * It builds a planar graph in which each face is labeled with the winding number
      * The overlap bit is needs to be set when the max winding number minus the min winding number is > 1
      * https://github.com/GraphiteEditor/Graphite/blob/master/libraries/path-bool/src/path_boolean.rs suggests that to do this one would need to go MajorGraph => MinorGraph => DualGraph
      * Sadly some ofthe steps use private functions so hacking on visibility might be required
      * _haven't had time to fully try this_
   * @raphlinus also suggested a Bently-Ottman style sweep line algorithm might work
      * AFAIK there isn't an off the shelf implementation that works with quads or cubics
      * _haven't tried this_
