"""Builds src/assets/ng-icons.woff2: the Segoe Fluent Icons code points the app uses, drawn with Microsoft's
open-source Fluent UI System Icons (MIT). Segoe only ships with Windows; this font is the fallback that
`--icons` falls to elsewhere (macOS), so the markup keeps its Segoe code points.

    python -m pip install fonttools brotli
    python scripts/build-icons.py <dir with FluentSystemIcons-Regular/Filled .ttf and .json>

From https://github.com/microsoft/fluentui-system-icons/tree/main/fonts. A new icon in the app needs a line here.
"""

import json
import sys
from pathlib import Path

from fontTools.subset import Options, Subsetter
from fontTools.ttLib import TTFont

# Segoe code point -> Fluent icon name; `:f` takes the filled style.
ICONS = {
    0xE70D: 'chevron_down', 0xE70E: 'chevron_up', 0xE710: 'add', 0xE711: 'dismiss', 0xE713: 'settings',
    0xE718: 'pin', 0xE71C: 'filter', 0xE721: 'search', 0xE72C: 'arrow_clockwise', 0xE72E: 'lock_closed',
    0xE734: 'star', 0xE735: 'star:f', 0xE738: 'subtract', 0xE73E: 'checkmark', 0xE73F: 'arrow_minimize',
    0xE740: 'full_screen_maximize', 0xE745: 'split_vertical', 0xE74A: 'arrow_up', 0xE74D: 'delete',
    0xE74F: 'speaker_mute', 0xE767: 'speaker_2', 0xE768: 'play:f', 0xE769: 'pause:f', 0xE76B: 'chevron_left',
    0xE76C: 'chevron_right', 0xE76F: 're_order_dots_horizontal', 0xE785: 'lock_open', 0xE786: 'video_clip_multiple',
    0xE787: 'calendar_ltr', 0xE7B3: 'eye', 0xE7F4: 'tv', 0xE80F: 'home', 0xE81C: 'history', 0xE893: 'next',
    0xE896: 'arrow_download', 0xE8A7: 'picture_in_picture', 0xE8B2: 'movies_and_tv', 0xE8B5: 'arrow_import',
    0xE8BB: 'dismiss', 0xE8C8: 'copy', 0xE8CB: 'arrow_sort', 0xE8D9: 'star_off', 0xE8EC: 'tag', 0xE8F1: 'library',
    0xE8FD: 'text_bullet_list_ltr', 0xE921: 'subtract', 0xE922: 'maximize', 0xE993: 'speaker_0', 0xE994: 'speaker_1',
    0xE995: 'speaker_2', 0xEA8F: 'alert', 0xEB51: 'heart', 0xEB52: 'heart:f', 0xED1A: 'eye_off',
    0xED3C: 'skip_back_10', 0xEDE1: 'arrow_export_ltr', 0xF0E2: 'grid',
}


def main(src: Path, out: Path):
    fonts, names = {}, {}
    for style in ('Regular', 'Filled'):
        fonts[style] = TTFont(src / f'FluentSystemIcons-{style}.ttf')
        names[style] = json.loads((src / f'FluentSystemIcons-{style}.json').read_text())
    base = fonts['Regular']
    glyf, hmtx = base['glyf'], base['hmtx']
    cmap = {}
    for point, spec in ICONS.items():
        name, _, filled = spec.partition(':')
        style = 'Filled' if filled else 'Regular'
        key = f'ic_fluent_{name}_20_{style.lower()}'
        if key not in names[style]:
            sys.exit(f'{key} is not in Fluent UI System Icons')
        font = fonts[style]
        glyph = font.getBestCmap()[names[style][key]]
        if style == 'Filled':
            # Bring the filled glyph into the regular font, under its own name.
            target = f'{glyph}.filled'
            if target not in glyf:
                glyf[target] = font['glyf'][glyph]
                hmtx[target] = font['hmtx'][glyph]
            glyph = target
        cmap[point] = glyph
    # `glyf` grew its glyph order with the filled glyphs.
    base.setGlyphOrder(glyf.glyphOrder)
    for table in base['cmap'].tables:
        table.cmap = dict(cmap) if table.isUnicode() else {}
    options = Options()
    options.flavor = 'woff2'
    options.name_IDs = []
    subsetter = Subsetter(options)
    subsetter.populate(glyphs=sorted(set(cmap.values())), unicodes=list(cmap))
    subsetter.subset(base)
    base['name'].setName('NG Icons', 1, 3, 1, 0x409)
    base.flavor = 'woff2'
    base.save(out)
    print(f'{out}: {len(cmap)} icons')


if __name__ == '__main__':
    main(Path(sys.argv[1]), Path(__file__).resolve().parent.parent / 'src' / 'assets' / 'ng-icons.woff2')
