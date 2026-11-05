"""Check the full-colour stacked PNG exports against the masters. Run: python3 logo/test_stacked_exports.py"""
from pathlib import Path

from PIL import Image

HERE = Path(__file__).parent
RED, INK, WHITE = (224, 65, 42), (23, 24, 28), (255, 255, 255)
MASTER_W, MASTER_H = 418.18, 345.75          # stacked master viewBox

for name, wordmark in (("crablink-stacked-800.png", INK), ("crablink-stacked-reversed-800.png", WHITE)):
    im = Image.open(HERE / "export/stacked" / name)
    assert im.mode == "RGBA", (name, im.mode)
    assert im.width == 800, (name, im.size)
    assert abs(im.height - round(800 * MASTER_H / MASTER_W)) <= 1, (name, im.size)
    assert im.getpixel((0, 0))[3] == 0, f"{name}: background not transparent"
    opaque = {p[:3] for _, p in im.getcolors(1 << 20) if p[3] == 255}
    assert RED in opaque, f"{name}: no Crab Red symbol"
    assert wordmark in opaque, f"{name}: wordmark colour {wordmark} missing"
    print("ok", name, im.size)
