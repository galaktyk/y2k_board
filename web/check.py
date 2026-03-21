from fontTools.ttLib import TTFont

font = TTFont("DejaVuSans.ttf")
cmap = font.getBestCmap()

targets = {
    "⟳": 0x27F3,
    "↻": 0x21BB,
    "▨": 0x25A8,
    "☐": 0x2610,
}

for char, cp in targets.items():
    print(f"{char} U+{cp:04X} → {'✓ present' if cp in cmap else '✗ missing'}")