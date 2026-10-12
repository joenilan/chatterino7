# Promo / demo video kit

Tools used to record the Jawjack Google-verification demo; reuse them for promo videos.

- `backdrop.html` — full-screen Zombie Digital gradient (pink / purple / cyan glow on a faint grid).
  Open it as a borderless Chrome app window behind the app, so nothing else on the desktop shows:
  `chrome --user-data-dir=<temp dir> --app=file:///…/backdrop.html --window-position=760,140 --window-size=1920,1120`
- `stage.ps1` — places a window so its visible frame is exactly a given size and position, and brings
  it to the front: `stage.ps1 -Process chat-workbench -X 920 -Y 270 -W 1600 -Height 900` (also `-Id`, `-Title`, `-List`).
- `act.ps1` — glides the real mouse cursor smoothly to a point before clicking (looks natural on camera),
  clicks inside Jawjack through its control channel (`click`, `move`, `type`, `key`), or clicks anywhere
  on screen (`osclick`). Needs a `cc.ps1` next to it that calls
  `chat-workbench.exe --control-call <session> <json>` for the Jawjack session being driven.
- `edit-example.py` — the edit used for the verification video: privacy covers (`drawbox`, timed to the
  source), per-clip speed, a zoom on one clip, lower-third captions with the gradient accent, crossfades
  between clips, and animated intro / end cards with Chomp. Needs ffmpeg, Pillow and cairosvg.

Recording: GPU desktop capture of the stage area, written in short segments so stopping never loses footage:

```
ffmpeg -f lavfi -i "ddagrab=output_idx=0:framerate=60:offset_x=760:offset_y=180:video_size=1920x1080:draw_mouse=1:dup_frames=1" \
  -vf "hwdownload,format=bgra" -c:v h264_nvenc -preset p5 -cq 17 -pix_fmt yuv420p -f segment -segment_time 5 seg%03d.mkv
```
Join the segments by concatenating their raw H.264 (`-bsf:v h264_mp4toannexb -f h264`) and remuxing with
`-fflags +genpts -r 60`; concatenating the .mkv files directly gives wrong timestamps.

Rules learned the hard way:
- Don't run a second Jawjack for demos. Every copy shares the Windows saved logins, and Twitch refresh
  tokens are one-use, so a second copy can sign the real one out. Record the real Jawjack in a temporary
  workspace, then close it.
- Use a fresh browser profile for sign-in screens (no tabs, no extensions), and cover emails, account
  lists, account IDs in URLs and link previews. Check every frame around transitions: pages flash old
  content while loading.
