#!/usr/bin/env python3
"""Builds the AI-loop test pages: one page per question kind, plus a diagram page.

    python3 -I tests/ai_loop/make_pages.py

Writes mc.png, tf.png, blank.png, short.png, matching.png, multipart.png, figure.png and
quiz_all_types.pdf (all seven pages, in that order) next to this script. The Rust test
tests/ai_loop.rs sends the PDF through VERSHION_IMPORT_PDF, so the pages are rasterised by the
app's own renderer and converted by the configured provider. Needs Pillow; the fixtures are
committed, so this only runs when they change.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
W, H = 1000, 1400
REG = "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf"
BOLD = "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Bold.ttf"
ITAL = "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Italic.ttf"


def page(name, draw_fn):
    img = Image.new("RGB", (W, H), "white")
    d = ImageDraw.Draw(img)
    d.text((70, 60), "Biology Quiz - Cell Structure", fill="black", font=ImageFont.truetype(BOLD, 34))
    d.line([(70, 105), (W - 70, 105)], fill="black", width=2)
    draw_fn(d)
    img.save(HERE / name)
    return img


def lines(d, y, rows, font_size=26, bold=False, italic=False, gap=44, indent=60):
    f = ImageFont.truetype(BOLD if bold else (ITAL if italic else REG), font_size)
    for row in rows:
        d.text((indent, y), row, fill="black", font=f)
        y += gap
    return y


def mc(d):
    y = lines(d, 150, ["1. Which organelle produces ATP in a eukaryotic cell?"], 28, bold=True)
    y = lines(d, y + 10, ["A. Nucleus", "B. Mitochondrion", "C. Ribosome", "D. Golgi apparatus"])
    lines(d, y + 20, ["Answer: B"], 24, italic=True)


def tf(d):
    y = lines(d, 150, ["2. The mitochondrion is the site of ATP synthesis."], 28, bold=True)
    y = lines(d, y + 20, ["( ) True", "( ) False"])
    lines(d, y + 20, ["Answer: True"], 24, italic=True)


def blank(d):
    y = lines(d, 150, ["3. The chemical symbol for sodium is ___."], 28, bold=True)
    y = lines(d, y + 20, ["4. Water is made of one oxygen and two ___ atoms."], 28, bold=True)
    lines(d, y + 20, ["Answers: Na; hydrogen"], 24, italic=True)


def short(d):
    y = lines(d, 150, ["5. Explain why ice floats on liquid water."], 28, bold=True)
    lines(d, y + 20, ["(three sentences)", "Answer: ice is less dense than water because its", "hydrogen bonds hold the molecules in an open lattice."], 24, italic=True)


def matching(d):
    y = lines(d, 150, ["6. Match each organelle with its job."], 28, bold=True)
    y = lines(d, y + 10, ["1. Nucleus            A. Packages proteins for export",
                          "2. Golgi apparatus     B. Holds the DNA",
                          "3. Ribosome            C. Builds proteins"], 24)
    lines(d, y + 20, ["Answers: 1-B, 2-A, 3-C"], 24, italic=True)


def multipart(d):
    y = lines(d, 150, ["7. A cell is placed in a salty solution."], 28, bold=True)
    y = lines(d, y + 10, ["(a) What happens to the cell? Explain.", "(b) Name the process responsible."], 26)
    lines(d, y + 20, ["Answers: (a) water leaves, the cell shrinks; (b) osmosis"], 24, italic=True)


def figure(d):
    y = lines(d, 150, ["8. The triangle below has legs 3 cm and 4 cm."], 28, bold=True)
    y = lines(d, y + 10, ["What is the length of the hypotenuse?"], 28, bold=True)
    # A labelled right triangle: 3 cm vertical, 4 cm horizontal, hypotenuse marked x.
    x0, y0 = 300, y + 40
    d.line([(x0, y0), (x0, y0 + 300)], fill="black", width=4)
    d.line([(x0, y0 + 300), (x0 + 400, y0 + 300)], fill="black", width=4)
    d.line([(x0, y0), (x0 + 400, y0 + 300)], fill="black", width=4)
    d.rectangle([x0, y0 + 280, x0 + 20, y0 + 300], outline="black", width=3)
    f = ImageFont.truetype(REG, 26)
    d.text((x0 - 100, y0 + 140), "3 cm", fill="black", font=f)
    d.text((x0 + 160, y0 + 315), "4 cm", fill="black", font=f)
    d.text((x0 + 210, y0 + 110), "x", fill="black", font=f)
    lines(d, y0 + 380, ["Answer: 5 cm"], 24, italic=True)


PAGES = [
    ("mc.png", mc),
    ("tf.png", tf),
    ("blank.png", blank),
    ("short.png", short),
    ("matching.png", matching),
    ("multipart.png", multipart),
    ("figure.png", figure),
]


def main():
    images = []
    for name, fn in PAGES:
        images.append(page(name, fn))
    first, rest = images[0], images[1:]
    first.save(HERE / "quiz_all_types.pdf", save_all=True, append_images=rest, resolution=150.0)
    print("wrote", ", ".join(name for name, _ in PAGES), "and quiz_all_types.pdf")


if __name__ == "__main__":
    main()
