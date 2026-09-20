#!/usr/bin/env python3
"""Repack reviewed pixel artwork. Pillow is an optional maintainer tool only.

Runtime assets are committed and embedded; neither Python nor Pillow is needed
at build or startup. Small marks use hand-aligned facets, with no glow.
"""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1] / 'assets'
BRAND = ROOT / 'branding'
PALETTE = {
    '.': (0, 0, 0, 0),
    'W': (240, 219, 255, 255), 'P': (201, 147, 250, 255),
    'V': (126, 88, 237, 255), 'B': (67, 94, 231, 255),
    'C': (100, 200, 255, 255), 'D': (32, 47, 117, 255),
    'N': (19, 26, 67, 255),
}
TINY = {
    8: [
        'W......P',
        'PV....VB',
        'BC....CD',
        '.BV..BD.',
        '.DC..VN.',
        '..BCVD..',
        '..DVB...',
        '...DN...',
    ],
    16: [
        '................',
        '.W............W.',
        '.PP..........PV.',
        '.VWP........PVB.',
        '.BCV........VCB.',
        '..BDV......BBD..',
        '..DCB......CDN..',
        '...BVC....BBD...',
        '...DBV....VDN...',
        '.P..DVC..BBD..P.',
        '.VB.DBV..VDN.PV.',
        '..CB.DVCBBD.VBD.',
        '..DBD.VWPVN.CD..',
        '...NBDDPVCDBN...',
        '..DBVBNVCVBDVD..',
        '.DNDVDBDNBDNDDD.',
    ],
}


def save(image, path):
    image.save(path, optimize=True)


def main():
    clean = Image.open(BRAND / 'source/clean.png').convert('RGBA')
    # Retain the subject bounds, discarding only transparent export padding.
    mask = clean.getchannel('A').point(lambda a: 255 if a >= 128 else 0)
    clean = clean.crop(mask.getbbox())
    for size in (24, 32, 64, 128):
        # The two small app marks use a separately simplified facet master.
        source = Image.open(BRAND / 'source/small.png') if size <= 32 else clean
        icon = source.resize((size, size), Image.Resampling.NEAREST)
        icon.putalpha(icon.getchannel('A').point(lambda a: 255 if a >= 128 else 0))
        save(icon, BRAND / f'crystal-{size}.png')
    for size, rows in TINY.items():
        assert len(rows) == size and all(len(row) == size for row in rows)
        icon = Image.new('RGBA', (size, size))
        icon.putdata([PALETTE[c] for row in rows for c in row])
        save(icon, BRAND / f'crystal-{size}.png')
    # The same export rectangle aligns the clean, subtle, and full variants.
    stages = []
    for name in ('clean', 'subtle', 'full'):
        sprite = Image.open(BRAND / 'source' / f'{name}.png').convert('RGBA')
        sprite = sprite.resize((160, 136), Image.Resampling.NEAREST)
        stage = Image.new('RGBA', (480, 272))
        stage.alpha_composite(sprite, (160, 44))
        stages.append(stage)
    clean, subtle, full = stages
    save(clean, ROOT / 'boot/clean.png')
    save(subtle, ROOT / 'boot/subtle.png')
    # First sweep reveals the cyan/blue left half; the violet side follows.
    cyan = subtle.copy()
    cyan.paste(full.crop((0, 0, 240, 272)), (0, 0))
    save(cyan, ROOT / 'boot/cyan.png')
    save(full, ROOT / 'boot/full.png')


if __name__ == '__main__':
    main()
