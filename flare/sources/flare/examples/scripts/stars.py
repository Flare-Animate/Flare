"""Python example: build a little animation from scratch.

Run from the app (Commands > Run Script...) or headless:
    cargo run -p ac-cli -- run examples/scripts/stars.py --gif stars.gif
"""
import flare as ac

ac.doc.set(width=550, height=400, background="#14213D")
ac.layer.set(name="Stars")
for i in range(6):
    ac.draw.polystar(cx=55 + i * 88, cy=120 + (i % 2) * 60, r=28, sides=5, star=True,
                     inner=0.45, fill="#FCA311")
ac.timeline.insert_keyframe(layer="Stars", frame=36)
ac.select.layer(layer="Stars")
ac.selection.move(dy=160)
ac.selection.rotate(degrees=144)
ac.timeline.set_tween(kind="classic", frame=1, ease=-60)

ac.layer.add(name="Title")
ac.draw.text(x=150, y=20, text="Made with Python", size=32, color="#E5E5E5")
info = ac.doc.info()
print(f"{info['frames']} frames, layers: {info['layers']}")
