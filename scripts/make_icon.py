# Draws the app icon (same design as ui/icon.svg) at 4x and downsamples.
import sys
from PIL import Image, ImageDraw

SS = 4
N = 1024 * SS
k = N / 636.0            # svg viewBox "-62 -62 636 636"
def p(v): return (v + 62) * k

img = Image.new("RGBA", (N, N), (0, 0, 0, 0))

def rrect_mask(x, y, w, h, r):
    m = Image.new("L", (N, N), 0)
    ImageDraw.Draw(m).rounded_rectangle([p(x), p(y), p(x + w), p(y + h)], radius=r * k, fill=255)
    return m

# background squircle with vertical gradient
grad = Image.new("RGBA", (N, N))
gd = ImageDraw.Draw(grad)
top, bot = (0x2a, 0x2a, 0x30), (0x12, 0x12, 0x14)
for yy in range(N):
    t = yy / (N - 1)
    gd.line([(0, yy), (N, yy)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bot)) + (255,))
img.paste(grad, (0, 0), rrect_mask(16, 16, 480, 480, 112))

def fill(x, y, w, h, r, color):
    layer = Image.new("RGBA", (N, N), color)
    img.paste(layer, (0, 0), rrect_mask(x, y, w, h, r))

fill(88, 88, 196, 336, 30, (0x9b, 0x8f, 0xf5, 255))
fill(300, 88, 124, 148, 30, (0xf0, 0x7a, 0xad, 255))

# hatched green tile
hatch = Image.new("RGBA", (N, N), (0xdf, 0xf5, 0xe8, 255))
hd = ImageDraw.Draw(hatch)
step, bar = 22 * k, 8 * k
i = -N
while i < 2 * N:
    hd.polygon([(i, 0), (i + bar, 0), (i + bar - N, N), (i - N, N)], fill=(0x3c, 0xcf, 0x7c, 255))
    i += step
img.paste(hatch, (0, 0), rrect_mask(300, 252, 124, 172, 30))
border = Image.new("L", (N, N), 0)
bd = ImageDraw.Draw(border)
bd.rounded_rectangle([p(300), p(252), p(424), p(424)], radius=30 * k, outline=255, width=int(8 * k))
img.paste(Image.new("RGBA", (N, N), (0x3c, 0xcf, 0x7c, 255)), (0, 0), border)

fill(112, 300, 148, 100, 20, (0x7c, 0x6f, 0xe0, 255))

img.resize((1024, 1024), Image.LANCZOS).save(sys.argv[1])
