from pathlib import Path
import argparse

from PIL import Image, ImageDraw, ImageFont, PngImagePlugin


def centered(draw, text, center, font, color):
    draw.text(center, text, font=font, fill=color, anchor="mm")


def render(font_path, output):
    output.mkdir(parents=True, exist_ok=True)
    font = lambda size: ImageFont.truetype(str(font_path), size)

    on_air = Image.new("RGB", (768, 256), "#090102")
    draw = ImageDraw.Draw(on_air)
    draw.rounded_rectangle((10, 10, 757, 245), radius=24, outline="#ff303a", width=8)
    centered(draw, "On Air", (384, 128), font(139), "#ff303a")

    credit = Image.new("RGB", (1024, 160), "#222d32")
    draw = ImageDraw.Draw(credit)
    draw.rectangle((4, 4, 1019, 155), outline="#b8aaa0", width=3)
    centered(draw, "Powered by Slofox", (512, 80), font(80), "#d3d9d4")

    for name, image in [("on-air", on_air), ("credit", credit)]:
        metadata = PngImagePlugin.PngInfo()
        metadata.add_text("Description", "Procedurally typeset Slofox studio sign")
        image.save(output / f"{name}.png", pnginfo=metadata, optimize=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Typeset the static On Air and credit textures (requires Pillow).")
    parser.add_argument("--font", type=Path, default=Path(__file__).resolve().parents[1] / "assets" / "studio" / "DejaVuSans-Bold.ttf")
    arguments = parser.parse_args()
    render(arguments.font, Path(__file__).resolve().parents[1] / "assets" / "studio")
