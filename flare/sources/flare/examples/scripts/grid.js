// JavaScript example (runs inside Flare, no install needed).
// Commands > Run Script... and pick this file.
ac.layer.add({ name: "Grid" });
for (let y = 0; y < 4; y++) {
  for (let x = 0; x < 6; x++) {
    const hue = (x * 40 + y * 25) % 255;
    ac.draw.rect({ x: 30 + x * 85, y: 40 + y * 85, w: 60, h: 60, radius: 12,
                   fill: `#${(hue).toString(16).padStart(2, "0")}66CC` });
  }
}
ac.select.layer({ layer: "Grid" });
ac.selection.to_symbol({ name: "Tile Grid", kind: "movieclip" });
print("Library now has", ac.library.list().length, "items");
