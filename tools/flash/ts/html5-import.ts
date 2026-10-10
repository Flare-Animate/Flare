// Adobe Animate HTML5 Canvas (CreateJS) export -> Flare import manifest. Pure, regex-based.
export interface Html5Symbol { name: string; frames: number; children: string[]; bitmaps: string[]; }
export interface Html5Manifest {
  format: "animate-html5"; width: number; height: number; fps: number; bg: string;
  assets: { id: string; src: string }[]; symbols: Html5Symbol[]; stage?: string;
}

const num = (s: string, k: string, d: number) => +(new RegExp(`\\b${k}\\s*:\\s*([\\d.]+)`).exec(s)?.[1] ?? d);

/** Frames of a timeline body: wait(n) + to(...,n) per tween chain, max over chains. */
function frames(body: string): number {
  let max = 1;
  for (const t of body.split("addTween").slice(1)) {
    let n = 0;
    for (const m of t.matchAll(/\.wait\((\d+)\)/g)) n += +m[1];
    for (const m of t.matchAll(/\.to\(\{[^]*?\}\s*,\s*(\d+)\)/g)) n += +m[1];
    max = Math.max(max, n);
  }
  return max;
}

export function parseHtml5(js: string): Html5Manifest {
  if (!/createjs|cjs\./.test(js) || !/lib\./.test(js)) throw new Error("not an Animate canvas export");
  const p = /lib\.properties\s*=\s*\{([^]*?)\n?\};/.exec(js)?.[1] ?? "";
  const assets = [...(/manifest\s*:\s*\[([^]*?)\]/.exec(p)?.[1] ?? "").matchAll(/\{[^}]*?src\s*:\s*"([^"]+)"[^}]*?id\s*:\s*"([^"]+)"[^}]*\}/g)]
    .map(m => ({ src: m[1], id: m[2] }));
  const bg = /color\s*:\s*"([^"]+)"/.exec(p)?.[1] ?? "#FFFFFF";
  const symbols: Html5Symbol[] = [];
  const starts = [...js.matchAll(/\(lib\.(\w+)\s*=\s*function\s*\([^)]*\)\s*\{/g)];
  starts.forEach((m, i) => {
    const body = js.slice(m.index! + m[0].length, starts[i + 1]?.index ?? js.length);
    const children = [...new Set([...body.matchAll(/new lib\.(\w+)\(/g)].map(x => x[1]))];
    symbols.push({
      name: m[1], frames: frames(body), children,
      bitmaps: children.filter(n => assets.some(a => a.id === n)),
    });
  });
  const stage = /(?:exportRoot|root)\s*=\s*new lib\.(\w+)\(/.exec(js)?.[1];
  return { format: "animate-html5", width: num(p, "width", 550), height: num(p, "height", 400), fps: num(p, "fps", 24), bg, assets, symbols, stage };
}

/** Script srcs referenced by an HTML export (CreateJS runtime excluded). */
export const scriptSrcs = (html: string): string[] =>
  [...html.matchAll(/<script[^>]+src="([^"]+\.js)"/gi)].map(m => m[1]).filter(s => !/createjs/i.test(s));
