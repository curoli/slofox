from pathlib import Path
import argparse

from PIL import Image, ImageDraw, ImageFont, PngImagePlugin


def centered(draw, text, center, font, color):
    draw.text(center, text, font=font, fill=color, anchor="mm")


def render(font_path, output):
    output.mkdir(parents=True, exist_ok=True)
    cream = "#fff0d6"
    font = lambda size: ImageFont.truetype(str(font_path), size)
    title = Image.new("RGB", (2048, 352), "#16242c")
    draw = ImageDraw.Draw(title)
    draw.rounded_rectangle((8, 8, 2039, 343), radius=18, outline="#bd955a", width=8)
    centered(draw, "AI Talks with Jati & Oliver", (1024, 132), font(118), cream)
    centered(draw, "Sonderausgabe: Iris hat Geburtstag", (1024, 264), font(66), "#f5bc75")

    on_air = Image.new("RGB", (768, 256), "#090102")
    draw = ImageDraw.Draw(on_air)
    draw.rounded_rectangle((10, 10, 757, 245), radius=24, outline="#ff303a", width=8)
    centered(draw, "On Air", (384, 128), font(139), "#ff303a")

    calendar = Image.new("RGB", (512, 704), cream)
    draw = ImageDraw.Draw(calendar)
    draw.rectangle((0, 0, 511, 160), fill="#a02e31")
    centered(draw, "OKTOBER", (256, 105), font(64), cream)
    centered(draw, "17", (256, 422), font(318), "#2c292b")
    centered(draw, "2026", (256, 620), font(80), "#a02e31")
    for horizontal in (105, 407):
        draw.rounded_rectangle((horizontal - 12, 16, horizontal + 12, 61), radius=10, fill="#bfc4c2")

    credit = Image.new("RGB", (1024, 160), "#222d32")
    draw = ImageDraw.Draw(credit)
    draw.rectangle((4, 4, 1019, 155), outline="#b8aaa0", width=3)
    centered(draw, "Powered by Slofox", (512, 80), font(80), "#d3d9d4")

    for name, image in [("title", title), ("on-air", on_air), ("calendar", calendar), ("credit", credit)]:
        metadata = PngImagePlugin.PngInfo()
        metadata.add_text("Description", "Procedurally typeset Slofox studio sign")
        image.save(output / f"{name}.png", pnginfo=metadata, optimize=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Typeset the physical studio sign textures (requires Pillow).")
    parser.add_argument("--font", type=Path, default=Path("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"))
    arguments = parser.parse_args()
    render(arguments.font, Path(__file__).resolve().parents[1] / "assets" / "studio")
