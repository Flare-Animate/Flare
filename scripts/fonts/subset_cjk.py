#!/usr/bin/env python3
"""Cuts the Noto Sans CJK fonts down to what the interface can use, for embedding.

The full fonts are 4 to 8 MB each. A subset keeps the characters of the national character set
of the language (JIS X 0208 for Japanese, GB 2312 for Chinese, KS X 1001 Hangul for Korean), so a
layer or file named in that language still shows, plus every character the language's files in
`localizations/` use, so a translation never loses one.

    python3 scripts/fonts/subset_cjk.py SOURCE_DIR assets/ui/fonts

SOURCE_DIR holds NotoSansJP-Regular.otf, NotoSansSC-Regular.otf and NotoSansKR-Regular.otf from
https://github.com/notofonts/noto-cjk (Sans/SubsetOTF). Needs `pip install fonttools`.
The output is committed; run this again when a localization uses a character not yet covered
(a test in flare-ui says which).
"""
import sys
from pathlib import Path

from fontTools import subset

ROOT = Path(__file__).resolve().parents[2]

# language folder in localizations/, source font, output name
FONTS = [
    ("ja", "NotoSansJP-Regular.otf", "NotoSansCJK-ja-subset.otf"),
    ("zh-CN", "NotoSansSC-Regular.otf", "NotoSansCJK-zh-subset.otf"),
    ("ko", "NotoSansKR-Regular.otf", "NotoSansCJK-ko-subset.otf"),
]


def rows(codec, lead_range, trail=(0xA1, 0xFE)):
    """Every character a double-byte legacy encoding has in the given lead-byte range."""
    out = set()
    for lead in range(lead_range[0], lead_range[1] + 1):
        for tail in range(trail[0], trail[1] + 1):
            try:
                out.add(bytes([lead, tail]).decode(codec))
            except UnicodeDecodeError:
                pass
    return out


def national_set(language):
    if language == "ja":
        chars = rows("euc_jp", (0xA1, 0xFE))
        chars |= {chr(c) for c in range(0x3040, 0x3100)}
    elif language == "zh-CN":
        chars = rows("gb2312", (0xA1, 0xF7))
    else:
        chars = {c for c in rows("euc_kr", (0xA1, 0xFD)) if not 0x4E00 <= ord(c) <= 0x9FFF}
        chars |= {chr(c) for c in range(0x3130, 0x3190)}
    return {c for c in chars if ord(c) >= 0x2000 or c.isascii()}


def localization_characters(language):
    text = "".join(p.read_text(encoding="utf-8") for p in (ROOT / "localizations" / language).glob("*.ftl"))
    # The names of all the languages, as the language list shows them.
    text += "日本語简体中文한국어"
    return set(text)


def main(source, out):
    source, out = Path(source), Path(out)
    ascii_and_punctuation = {chr(c) for c in range(0x20, 0x7F)}
    ascii_and_punctuation |= {chr(c) for c in range(0x2010, 0x2028)}
    ascii_and_punctuation |= {chr(c) for c in range(0x3000, 0x3040)}
    ascii_and_punctuation |= {chr(c) for c in range(0xFF01, 0xFF5F)}
    ascii_and_punctuation |= {"·", "×", "…", "→", "←"}
    for language, name, target in FONTS:
        chars = ascii_and_punctuation | national_set(language) | localization_characters(language)
        options = subset.Options()
        options.layout_features = ["kern", "vert", "vrt2"]
        options.hinting = False
        options.notdef_outline = True
        options.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14]
        font = subset.load_font(str(source / name), options)
        subsetter = subset.Subsetter(options)
        subsetter.populate(unicodes=sorted(ord(c) for c in chars))
        subsetter.subset(font)
        subset.save_font(font, str(out / target), options)
        size = (out / target).stat().st_size
        print(f"{target}: {len(chars)} characters, {size / 1024:.0f} KiB")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
