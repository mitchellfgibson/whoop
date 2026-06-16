from PIL import Image, ImageDraw
import math

S = 1024
img = Image.new("RGBA", (S, S), (0,0,0,0))
d = ImageDraw.Draw(img)

# Background — vertical gradient: deep night-green sky -> darker sea, app palette.
top = (13, 21, 18)      # #0D1512 surfaceRaised
bot = (4, 8, 6)         # near-black
for y in range(S):
    t = y / S
    r = int(top[0]*(1-t) + bot[0]*t)
    g = int(top[1]*(1-t) + bot[1]*t)
    b = int(top[2]*(1-t) + bot[2]*t)
    d.line([(0,y),(S,y)], fill=(r,g,b,255))

accent   = (24, 201, 139)   # #18C98B
accent2  = (47, 230, 168)   # #2FE6A8 brighter
sailwhite= (228, 240, 235)
hullcol  = (15, 35, 28)
mast     = (190, 205, 198)

cx = S//2

# Sea: a calm band of waves in the lower third.
sea_y = int(S*0.70)
d.rectangle([0, sea_y, S, S], fill=(7, 16, 13, 255))
# wave lines
for i, yy in enumerate(range(sea_y+30, S, 70)):
    amp = 14 + i*2
    pts = []
    for x in range(0, S+1, 16):
        pts.append((x, yy + int(amp*math.sin(x/90.0 + i))))
    d.line(pts, fill=(24, 201, 139, 90), width=6)

# Mast
mast_top = int(S*0.16)
mast_bot = sea_y - 10
d.line([(cx, mast_top), (cx, mast_bot)], fill=mast, width=14)

# Main sail (big triangle, right of mast) — bright accent
d.polygon([(cx+18, mast_top+30), (cx+18, mast_bot-70), (cx+330, mast_bot-70)], fill=accent2)
# Fore sail (left of mast) — softer
d.polygon([(cx-18, mast_top+110), (cx-18, mast_bot-70), (cx-250, mast_bot-70)], fill=accent)
# Sail seams
for k in range(1,4):
    yy = mast_top+30 + (mast_bot-70 - (mast_top+30))*k/4
    d.line([(cx+18, yy), (cx+330 - (cx+330-(cx+18))*0 , yy)], fill=(13,40,30,120), width=4)

# Hull — a rounded boat shape sitting on the sea line
hull_top = sea_y - 70
hw = 360
d.polygon([
    (cx-hw, hull_top),
    (cx+hw, hull_top),
    (cx+hw-70, hull_top+120),
    (cx-hw+70, hull_top+120),
], fill=hullcol)
# Hull accent trim
d.line([(cx-hw, hull_top), (cx+hw, hull_top)], fill=accent2, width=10)

# Small flag at the masthead
d.polygon([(cx, mast_top), (cx+90, mast_top+22), (cx, mast_top+44)], fill=accent2)

# Round the corners with a superellipse-ish mask (iOS will mask anyway, but keep it clean)
img.save("/tmp/ship_1024.png")
print("wrote /tmp/ship_1024.png", img.size)
