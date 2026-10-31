// Test Movie in the page: Ruffle, the Flash player, as WebAssembly in an overlay over the editor.
//
// The Rust side publishes the movie and calls `globalThis.flareRuffle.play(bytes)`. Ruffle's
// script (`vendor/ruffle/ruffle.js`, from the @ruffle-rs/ruffle package) is only fetched on the first
// play, so a page that never tests a movie never downloads its 15 MB of WebAssembly. The overlay is
// closed by its button, by Escape, and by a second play, which replaces the movie.

const RUFFLE_SCRIPT = "vendor/ruffle/ruffle.js";

/** Loads Ruffle once and gives back the `RufflePlayer` namespace it registers. */
function loadRuffle(doc, win, base) {
  if (win.RufflePlayer?.newest) return Promise.resolve(win.RufflePlayer);
  // Where Ruffle finds its own WebAssembly and chunks, set before its script runs.
  win.RufflePlayer = { config: { publicPath: `${base}vendor/ruffle/`, autoplay: "on" } };
  return new Promise((resolve, reject) => {
    const script = doc.createElement("script");
    script.src = `${base}${RUFFLE_SCRIPT}`;
    script.onload = () => {
      if (win.RufflePlayer?.newest) resolve(win.RufflePlayer);
      else reject(new Error("Ruffle's script loaded but did not register a player"));
    };
    script.onerror = () => {
      win.RufflePlayer = undefined; // let a later play try again
      reject(new Error("Ruffle could not be loaded"));
    };
    doc.head.appendChild(script);
  });
}

/**
 * `doc` and `win` are the page's document and window; `base` is the folder the page is served
 * from, with a trailing slash. They are parameters so a test can give fakes.
 */
export function createFlareRuffle({ doc = document, win = window, base = new URL("./", import.meta.url).href } = {}) {
  let overlay = null;
  let onKey = null;

  function close() {
    if (onKey) doc.removeEventListener("keydown", onKey);
    onKey = null;
    overlay?.remove();
    overlay = null;
  }

  function open() {
    close();
    overlay = doc.createElement("div");
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-label", "Test Movie");
    overlay.style.cssText =
      "position:fixed;inset:0;z-index:10;display:flex;flex-direction:column;align-items:center;" +
      "justify-content:center;gap:12px;background:rgba(0,0,0,.72)";
    const stage = doc.createElement("div");
    stage.style.cssText = "width:min(90vw,1280px);height:min(80vh,720px);background:#000";
    const button = doc.createElement("button");
    button.textContent = "Close";
    button.style.cssText = "font:14px sans-serif;padding:6px 18px";
    button.addEventListener("click", close);
    overlay.append(stage, button);
    doc.body.appendChild(overlay);
    onKey = (event) => {
      if (event.key === "Escape") close();
    };
    doc.addEventListener("keydown", onKey);
    return stage;
  }

  /** Plays the SWF in `bytes`; rejects with a message the app can show. */
  async function play(bytes) {
    const stage = open();
    const mine = overlay;
    try {
      const ruffle = await loadRuffle(doc, win, base);
      if (overlay !== mine) return; // closed, or replaced, while Ruffle was loading
      const player = ruffle.newest().createPlayer();
      player.style.cssText = "width:100%;height:100%";
      stage.appendChild(player);
      await player.load({ data: bytes });
    } catch (error) {
      if (overlay === mine) close();
      throw error;
    }
  }

  return { play, close, isOpen: () => overlay !== null };
}
