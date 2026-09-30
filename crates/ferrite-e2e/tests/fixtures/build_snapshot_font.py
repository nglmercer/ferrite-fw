"""Build the original three-glyph downloadable font used by snapshot tests.

Development only: fonttools==4.66.1. Rust tests use the committed TTF and need
neither Python nor fontTools. No installed or third-party font is copied.
"""
from pathlib import Path
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

font = FontBuilder(1000, isTTF=True)
font.setupGlyphOrder([".notdef", "space", "F"])
font.setupCharacterMap({32: "space", 70: "F"})
glyphs = {}
for name in [".notdef", "space", "F"]:
    pen = TTGlyphPen(None)
    if name != "space":
        # Original stepped polygon: wide advance makes the loaded width clearly
        # different from a monospace fallback without depending on its pixels.
        for command, point in [(pen.moveTo, (100, 0)), (pen.lineTo, (100, 800)),
                               (pen.lineTo, (800, 800)), (pen.lineTo, (800, 600)),
                               (pen.lineTo, (350, 600)), (pen.lineTo, (350, 450)),
                               (pen.lineTo, (700, 450)), (pen.lineTo, (700, 250)),
                               (pen.lineTo, (350, 250)), (pen.lineTo, (350, 0))]:
            command(point)
        pen.closePath()
    glyphs[name] = pen.glyph()
font.setupGlyf(glyphs)
font.setupHorizontalMetrics({".notdef": (1000, 100), "space": (500, 0), "F": (1000, 100)})
font.setupHorizontalHeader(ascent=800, descent=-200)
font.setupNameTable({"familyName": "FerriteSnapshotProbe", "styleName": "Regular",
                   "uniqueFontIdentifier": "FerriteSnapshotProbe-Regular-1",
                   "fullName": "FerriteSnapshotProbe Regular", "psName": "FerriteSnapshotProbe-Regular",
                   "version": "Version 1.0"})
font.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
font.setupPost()
font.font.recalcTimestamp = False
font.font["head"].created = font.font["head"].modified = 2082844800  # 1970-01-01
font.save(Path(__file__).with_name("snapshot-probe.ttf"))
