// Browser UI: pick Animate HTML5 export .js/.html -> symbol/frame table.
import { parseHtml5 } from "../ts/html5-import.ts";
declare const document: any, FileReader: any, navigator: any, location: any;
export function render(js: string): string {
  const m = parseHtml5(js);
  const rows = m.symbols.map(s => `<tr><td>${s.name}</td><td>${s.frames}</td><td>${s.children.length}</td></tr>`).join("");
  return `<p>${m.width}x${m.height} @${m.fps}fps, ${m.symbols.length} symbols, ${m.assets.length} assets</p>` +
    `<table><tr><th>Symbol</th><th>Frames</th><th>Children</th></tr>${rows}</table>`;
}
if (typeof document !== "undefined") {
  const out = document.getElementById("out");
  document.getElementById("f").onchange = (e: any) => {
    const r = new FileReader();
    r.onload = () => { try { out.innerHTML = render(String(r.result)); } catch (x: any) { out.textContent = x.message; } };
    r.readAsText(e.target.files[0]);
  };
  if ("serviceWorker" in navigator && location.protocol.startsWith("http")) navigator.serviceWorker.register("sw.js");
}
