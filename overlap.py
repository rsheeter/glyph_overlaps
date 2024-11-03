"""Experiment with overlap detection.

Operates on UFO glif files as the font I wanted to play with happens to be UFO+designspace.

Usage:
	# setup
	python3 -m venv venv
	source venv/bin/activate
	pip install -r requirements.txt
	
	# run
	python overlap.py path/to/file.glif
"""

from dataclasses import dataclass
from lxml import etree
from pathlib import Path
from PIL import Image
import subprocess
import sys


@dataclass
class GlifPoint:
	x: int
	y: int
	typ: str


def read_points(contour):
	# -y to flip to svg coords
	return list(GlifPoint(int(point.attrib["x"]), -int(point.attrib["y"]), point.attrib.get("type", None)) for point in contour.xpath("./point"))


def process_glif_file(glif_file):
	tree = etree.parse(glif_file)

	contour_starts = set()
	points = []
	for contour in tree.xpath("/glyph/outline/contour"):
		contour_points = read_points(contour)
		assert contour_points[0].typ is not None, "TODO: rotate"

		contour_starts.add(len(points))
		if contour_points[-1].typ is None:
			# wrap back to start; we made sure start was typ'd
			contour_points.append(contour_points[0])

		points.extend(contour_points)

	svg_cmds = []
	offcurves = []
	for (i, pt) in enumerate(points):
		#print(pt)

		if i in contour_starts:
			svg_cmds.append((f"M{pt.x}, {pt.y}"))
		else:
			if pt.typ == "line":
				svg_cmds.append(f"L{pt.x},{pt.y}")
			elif pt.typ == "qcurve":
				if offcurves:
					# if there are lots of off-curves insert on-curves in between
					for insert_at in reversed(range(1, len(offcurves), 2)):
						before = offcurves[insert_at - 1]
						after = offcurves[insert_at]
						mid = GlifPoint((before.x + after.x) / 2, (before.y + after.y) / 2, "qcurve")
						offcurves.insert(insert_at, mid)
					offcurves.append(pt)

					# we should now have a series of off/on
					assert len(offcurves) % 2 == 0, f"{offcurves}"
					for i in range(0, len(offcurves), 2):
						off = offcurves[i]
						on = offcurves[i + 1]
						assert (None, "qcurve") == (off.typ, on.typ), f"off {off} on {on}"
						svg_cmds.append(f"Q{off.x},{off.y} {on.x},{on.y}")
				else:
					# if there are no off-curves degrade to line
					svg_cmds.append(f"L{pt.x},{pt.y}")

				offcurves = []
			elif pt.typ == "offcurve" or pt.typ is None:
				offcurves.append(pt)

		last = pt

	assert len(offcurves) == 0, offcurves

	minx = min(pt.x for pt in points) * 1.1
	miny = min(pt.y for pt in points) * 1.1
	maxx = max(pt.x for pt in points) * 1.1
	maxy = max(pt.y for pt in points) * 1.1
	for fill_rule in ("evenodd", "nonzero"):
		svg = "\n".join((
			f"<svg viewBox=\"{minx} {miny} {maxx - minx} {maxy - miny}\" xmlns=\"http://www.w3.org/2000/svg\">",
			"  <path fill-rule=\"{}\" d=\"{}\"/>".format(fill_rule, " ".join(svg_cmds)),
			"</svg>"
		))
		svg_file = Path(f"/tmp/overlap.{fill_rule}.svg")
		with open(svg_file, "w") as f:
			f.write(svg)

		png_file = svg_file.with_suffix(".png")
		subprocess.run(("resvg", svg_file, png_file), check=True)
		image = Image.open(png_file)
		colors = image.getcolors()
		print(png_file, image.mode)
		for color in colors:
			print("  ", color)


def main():
	glif_dir = Path.home() / "oss" / "googlesans-flex" / "sources" / "GoogleSansFlex-wg400-wd100-oz6-GD0-RD0-sl-10.ufo" / "glyphs"

	for glif_file in sys.argv[1:]:
		process_glif_file(Path(glif_file))
	


if __name__ == "__main__":
    main()