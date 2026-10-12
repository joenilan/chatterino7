# Example from the Google verification demo (2026-10-11): timings, covers and captions are
# specific to that take; copy it and adjust for a new video.
"""Jawjack Google-verification demo, take 2: privacy covers, speed-ups, a zoom
on the permission pop-up, styled captions, crossfades, intro and end cards."""
import io, os, subprocess, textwrap
from PIL import Image, ImageDraw, ImageFont, ImageFilter
import cairosvg

V = os.path.dirname(os.path.abspath(__file__)); os.chdir(V)
SRC = 'take2.mp4'
BRAND = r'E:\git\chatterino7\recreation\assets\brand'
F_REG, F_SEMI, F_BOLD = (r'C:\Windows\Fonts\segoeui.ttf', r'C:\Windows\Fonts\seguisb.ttf', r'C:\Windows\Fonts\segoeuib.ttf')
W, H, FPS, XF = 1920, 1080, 30, 0.4
INK, LILAC, PINK, CYAN = (236, 238, 241), (169, 156, 244), (236, 72, 153), (103, 232, 249)

def ff(args):
    r = subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y'] + args, capture_output=True, text=True)
    if r.returncode: raise SystemExit(r.stderr[-3000:])

def dur(path):
    out = subprocess.run(['ffprobe', '-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', path], capture_output=True, text=True).stdout
    return float(out.strip())

def mark(size, which='mark'):
    svg = os.path.join(BRAND, f'jawjack-{which}.svg')
    return Image.open(io.BytesIO(cairosvg.svg2png(url=svg, output_width=size, output_height=size))).convert('RGBA')

# ---- overlays -------------------------------------------------------------
def overlay(name, step, title, body, header=True):
    im = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    if header:
        m = mark(44)
        im.alpha_composite(m, (24, 22))
        d.text((78, 44), 'Jawjack', font=ImageFont.truetype(F_BOLD, 26), fill=INK, anchor='lm')
        d.text((190, 44), 'Sign in with Google for YouTube live chat', font=ImageFont.truetype(F_REG, 22), fill=(170, 174, 186), anchor='lm')
    # lower third: card with a gradient accent edge
    fb, ft = ImageFont.truetype(F_REG, 27), ImageFont.truetype(F_SEMI, 31)
    lines = textwrap.wrap(body, 80)
    card_w = 1180
    card_h = 34 + 44 + 36 * len(lines) + 22
    x0, y1 = 96, H - 56
    y0 = y1 - card_h
    shadow = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle([x0 + 6, y0 + 10, x0 + card_w + 6, y1 + 10], 22, fill=(0, 0, 0, 150))
    im.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(14)))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([x0, y0, x0 + card_w, y1], 22, fill=(16, 13, 24, 238), outline=(255, 255, 255, 28), width=1)
    grad = Image.new('RGBA', (6, card_h - 28))
    for y in range(card_h - 28):
        t = y / max(1, card_h - 29)
        c = tuple(int(a + (b - a) * t) for a, b in zip(PINK, CYAN))
        for x in range(6): grad.putpixel((x, y), (*c, 255))
    im.alpha_composite(grad, (x0 + 18, y0 + 14))
    tx = x0 + 44
    if step:
        d.ellipse([tx, y0 + 26, tx + 40, y0 + 66], fill=(*LILAC, 255))
        d.text((tx + 20, y0 + 46), str(step), font=ImageFont.truetype(F_BOLD, 24), fill=(22, 18, 31), anchor='mm')
        d.text((tx + 56, y0 + 46), title, font=ft, fill=INK, anchor='lm')
    else:
        d.text((tx, y0 + 46), title, font=ft, fill=INK, anchor='lm')
    for i, line in enumerate(lines):
        d.text((tx, y0 + 80 + 36 * i), line, font=fb, fill=(196, 199, 210))
    im.save(name)

# ---- privacy covers (absolute source times) ---------------------------------
OMNI = (740, 161, 632, 24, '0x272727')
COVERS = [
    (*OMNI, 32.5, 122.8),                                   # address bar: account IDs, sign-in code
    (972, 383, 382, 269, '0x0d0d0d', 32.5, 51.7),           # account / channel list
    (660, 478, 218, 26, '0xffffff', 51.6, 62.15),            # developer email (warning)
    (660, 652, 218, 28, '0xffffff', 51.6, 62.15),            # developer email after Advanced
    (420, 944, 345, 28, '0x1f1f1f', 51.6, 62.15),            # link preview (account ID)
    (1028, 498, 220, 26, '0x744100', 62.0, 79.85),          # email in the orange notice
    (1028, 498, 220, 26, '0x744100', 94.95, 98.4),
    (1028, 300, 222, 226, '0x744100', 98.2, 98.8),          # during the scroll
    (1028, 308, 220, 26, '0x744100', 98.3, 113.5),          # after the scroll
    (1024, 494, 228, 34, '0x422707', 113.45, 114.0),        # page redraw flash while loading
]
def covers():
    return ','.join(f"drawbox=x={x}:y={y}:w={w}:h={h}:color={c}@1:t=fill:enable='between(t,{a},{b})'" for x, y, w, h, c, a, b in COVERS)

parts = []
def clip(i, start, end, step, title, body, speed=1.0, zoom=None, header=True):
    ov = f'ov{i}.png'; overlay(ov, step, title, body, header)
    out = f'q{i:02d}.mp4'
    z = f',crop={zoom[2]}:{zoom[3]}:{zoom[0]}:{zoom[1]},scale={W}:{H}:flags=lanczos' if zoom else ''
    speed *= 1.5
    graph = (f'[0:v]{covers()}{z},setpts=(PTS-STARTPTS)/{speed},fps={FPS},format=rgba[v];'
             f'[v][1:v]overlay=0:0,format=yuv420p[o]')
    ff(['-ss', str(start), '-to', str(end), '-copyts', '-i', SRC, '-i', ov, '-filter_complex', graph, '-map', '[o]',
        '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-r', str(FPS), out])
    parts.append(out)

def card(i, seconds, draw):
    png = f'card{i}.png'
    im = Image.new('RGB', (W, H), (11, 9, 18))
    glow = Image.new('RGB', (W, H), (11, 9, 18)); g = ImageDraw.Draw(glow)
    g.ellipse([-300, -300, 900, 700], fill=(70, 22, 60)); g.ellipse([1200, 500, 2300, 1400], fill=(16, 60, 72)); g.ellipse([500, 150, 1500, 950], fill=(52, 28, 96))
    im = Image.blend(im, glow.filter(ImageFilter.GaussianBlur(160)), 0.9)
    draw(im)
    im.save(png)
    out = f'q{i:02d}.mp4'
    frames = int(seconds * FPS)
    ff(['-loop', '1', '-i', png, '-vf', f"zoompan=z='1+0.04*on/{frames}':x='iw/2-(iw/zoom/2)':y='ih/2-(ih/zoom/2)':d={frames}:s={W}x{H}:fps={FPS},format=yuv420p",
        '-frames:v', str(frames), '-c:v', 'libx264', '-preset', 'slow', '-crf', '16', out])
    parts.append(out)

def intro(im):
    d = ImageDraw.Draw(im)
    m = mark(340); im.paste(m, (W // 2 - 170, 150), m)
    d.text((W // 2, 590), 'Jawjack', font=ImageFont.truetype(F_BOLD, 92), fill=INK, anchor='mm')
    d.text((W // 2, 668), 'Native live-chat client for Twitch and YouTube', font=ImageFont.truetype(F_REG, 36), fill=(196, 199, 210), anchor='mm')
    d.rounded_rectangle([W // 2 - 330, 740, W // 2 + 330, 800], 30, outline=(*LILAC,), width=2)
    d.text((W // 2, 770), 'How Jawjack uses Google sign-in and YouTube', font=ImageFont.truetype(F_SEMI, 26), fill=LILAC, anchor='mm')
    d.text((W // 2, H - 70), 'zombie.digital/apps/jawjack', font=ImageFont.truetype(F_REG, 26), fill=(150, 154, 168), anchor='mm')

def outro(im):
    d = ImageDraw.Draw(im)
    m = mark(170); im.paste(m, (W // 2 - 85, 90), m)
    d.text((W // 2, 320), 'What Jawjack does with YouTube', font=ImageFont.truetype(F_BOLD, 58), fill=INK, anchor='mm')
    rows = [('Reads', 'live chat of streams you open', 'videos.list · liveChatMessages.streamList'),
            ('Posts', 'only the messages you type and send', 'liveChatMessages.insert'),
            ('Never', 'uploads, edits, deletes, rates, comments or captions', 'one scope: youtube.force-ssl'),
            ('Keeps', 'your sign-in on your PC only; Jawjack has no servers', 'Windows Credential Manager')]
    y = 420
    for verb, what, api in rows:
        d.rounded_rectangle([360, y, 1560, y + 92], 18, fill=(22, 18, 34), outline=(255, 255, 255, ), width=1)
        d.text((400, y + 46), verb, font=ImageFont.truetype(F_BOLD, 32), fill=LILAC, anchor='lm')
        d.text((560, y + 32), what, font=ImageFont.truetype(F_SEMI, 28), fill=INK, anchor='lm')
        d.text((560, y + 64), api, font=ImageFont.truetype(F_REG, 22), fill=(150, 154, 168), anchor='lm')
        y += 108
    d.text((W // 2, H - 60), 'Privacy policy: zombie.digital/apps/jawjack/privacy  ·  Revoke access any time: myaccount.google.com/permissions',
           font=ImageFont.truetype(F_REG, 24), fill=(170, 174, 186), anchor='mm')

card(0, 5, intro)
clip(1, 12.5, 31.6, 1, 'Add a YouTube live stream', 'The user pastes a YouTube live link into Jawjack (Add channel, Ctrl+K). Jawjack asks them to connect YouTube.')
clip(2, 33.0, 51.6, 2, 'Google sign-in, in the browser', 'Connect YouTube opens Google sign-in in the user\'s browser. The user picks the channel to use (other accounts hidden here for privacy).', speed=1.6)
clip(3, 51.6, 63.4, 3, 'Expected while Google reviews Jawjack', 'Until verification is complete Google shows this notice. The user opens Advanced and continues to Jawjack.')
clip(4, 63.0, 79.7, 4, 'Consent screen', 'Jawjack, its logo and its privacy policy and terms are shown. The user opens the details of the access requested.', speed=1.3)
clip(5, 80.2, 94.8, 5, 'One permission: YouTube (youtube.force-ssl)', 'Used only to read the live chat of streams the user opens, and to post the chat messages the user types. Nothing else.',
     zoom=(400, 268, 1120, 630), header=False)
clip(6, 95.0, 114.0, 6, 'The user continues', 'Access is granted only after the user clicks Continue.', speed=1.5)
clip(7, 114.0, 122.6, 7, 'Signed in', 'The sign-in is saved only on this PC (Windows Credential Manager). Jawjack has no servers.')
clip(8, 122.6, 139.2, 8, 'Live chat in Jawjack', 'Jawjack finds the stream\'s live chat (videos.list) and receives messages over one streaming connection (liveChatMessages.streamList).')
clip(9, 139.2, 156.0, 9, 'Disconnect any time', 'Settings, Connections: Sign out removes the sign-in from this PC. Access can also be revoked at myaccount.google.com/permissions.')
card(10, 9, outro)

# crossfade everything together
inputs, chain, offset, last = [], [], 0.0, '0:v'
durs = [dur(p) for p in parts]
for p in parts: inputs += ['-i', p]
for k in range(1, len(parts)):
    offset += durs[k - 1] - XF
    out = f'x{k}'
    chain.append(f'[{last}][{k}:v]xfade=transition=fade:duration={XF}:offset={offset:.3f}[{out}]')
    last = out
ff(inputs + ['-f', 'lavfi', '-i', 'anullsrc=r=48000:cl=stereo', '-filter_complex', ';'.join(chain), '-map', f'[{last}]', '-map', f'{len(parts)}:a',
     '-c:v', 'libx264', '-preset', 'slow', '-crf', '17', '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-shortest', '-movflags', '+faststart', 'jawjack-youtube-demo.mp4'])
print('ok', round(dur('jawjack-youtube-demo.mp4'), 1))
