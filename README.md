# glyph_overlaps

Usage

```shell
$ cargo run -- path/to/file.glif
$ cargo run -- path/to/dir.ufo
$ cargo run -- path/to/file.designspace
```

TODO:

Raph suggested https://github.com/GraphiteEditor/Graphite/tree/master/libraries/path-bool
likely has all the parts needed to detect differences in winding. It builds a planar graph 
in which each face is labeled with the winding number. The overlap bit is needs to be set 
when the max winding number minus the min winding number is > 1.

https://github.com/GraphiteEditor/Graphite/blob/master/libraries/path-bool/src/path_boolean.rs
suggests that to do this one would need to go MajorGraph => MinorGraph => DualGraph but some of
the steps use private functions so hacking on visibility might be required.
