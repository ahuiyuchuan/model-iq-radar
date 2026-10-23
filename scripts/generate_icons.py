"""生成随应用打包的探针 ICO/PNG；依赖 Pillow，执行 python scripts/generate_icons.py。作者：ahui。"""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

output = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"
output.mkdir(parents=True, exist_ok=True)
image = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((12, 12, 500, 500), radius=116, fill="#192b40", outline="#406180", width=6)
glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
halo = ImageDraw.Draw(glow)
halo.ellipse((105, 105, 407, 407), outline="#59bdff", width=15)
image = Image.alpha_composite(image, glow.filter(ImageFilter.GaussianBlur(19)))
draw = ImageDraw.Draw(image)
draw.ellipse((104, 104, 408, 408), outline="#a5ddff", width=14)
draw.ellipse((162, 162, 350, 350), outline="#74b8fb", width=14)
draw.ellipse((226, 226, 286, 286), fill="#b9e6ff")
draw.line((367, 144, 412, 99), fill="#72b9ff", width=13)
for size in (32, 128):
    image.resize((size, size), Image.Resampling.LANCZOS).save(output / f"{size}x{size}.png")
image.save(output / "icon.ico", sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
