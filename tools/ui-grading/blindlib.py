"""Shared pieces for building and verifying the blind grading set.

The one rule this file exists to enforce: a blind image is a pure function of
(source path, screen name, side, view). `make_blind.py` records that function in the key and
`verify_blind.py` replays it, so a key and a directory can never drift apart the way round 3's
did (6 of 21 pairs were graded against the wrong screen, and the "same image twice" control
was two different images).
"""
from PIL import Image

S = 1.7
K = 1440 / 2176

# Mask rectangles, in the source image's own pixels (before the S upscale).
OURS = [(33, 21, 74, 63)]        # our sidebar's Q badge
OURS7 = [(26, 20, 60, 62)]
LOGO = [(18, 10, 190, 58)]       # the reference's wordmark
FOOT = [(620, 755, 890, 797)]
PARROT = [(18, 8, 66, 58)]
LOGO7 = [(15, 4, 135, 62)]
FOOT7 = [(245, 752, 520, 797)]


def masks(name, side, narrow):
    """The rectangles to blank for one side of one screen. `side` is 'ours' or 'ref'."""
    if side == 'ours':
        return OURS7 if narrow else OURS
    if narrow:
        if name in ('banks', 'exams', 'imports'):
            return LOGO7 + FOOT7
        if name in ('exam', 'question-editor-exam', 'export'):
            return PARROT
        return []
    if name in ('banks', 'exams', 'imports'):
        return LOGO + FOOT
    if name in ('bank', 'question-editor-bank'):
        return LOGO
    return PARROT


def build(path, name, side, narrow):
    """The blind image: masks blanked with the colour beside them, then scaled to 1440 wide."""
    im = Image.open(path).convert('RGB')
    for x0, y0, x1, y1 in masks(name, side, narrow):
        b = (int(x0 * S), int(y0 * S), int(x1 * S), int(y1 * S))
        c = im.getpixel((min(b[2] + 6, im.width - 1), b[1] + (b[3] - b[1]) // 2))
        im.paste(c, b)
    return im.resize((round(im.width * K), round(im.height * K)), Image.LANCZOS)


def save(im, path):
    im.save(path, 'PNG', optimize=True)
