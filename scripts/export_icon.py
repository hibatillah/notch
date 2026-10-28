"""Export the Windows icon from assets/branding/logo.png; run through export-icon.sh."""

from pathlib import Path

from PIL import Image, ImageOps


project_directory = Path(__file__).resolve().parent.parent
branding_directory = project_directory / "assets" / "branding"
source = Image.open(branding_directory / "logo.png").convert("RGBA")

# Ignore faint stray pixels in the generated PNG when measuring its transparent margins.
bounds = source.getchannel("A").point(lambda alpha: 255 if alpha >= 128 else 0).getbbox()
if bounds is None:
    raise ValueError("The logo has no visible pixels")

left, top, right, bottom = bounds
source = source.crop((max(0, left - 2), max(0, top - 2), min(source.width, right + 2), min(source.height, bottom + 2)))
artwork = ImageOps.contain(source, (224, 224), Image.Resampling.LANCZOS)
icon = Image.new("RGBA", (256, 256))
# A small optical adjustment balances the heavier vertical stem on the left.
icon.alpha_composite(artwork, ((256 - artwork.width) // 2 + 4, (256 - artwork.height) // 2))

sizes = [(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)]
destination = branding_directory / "logo.ico"
icon.save(destination, format="ICO", sizes=sizes, bitmap_format="bmp")
print(f"Exported {destination} ({', '.join(str(width) for width, _ in sizes)} px)")
