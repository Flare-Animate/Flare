// Loads the WebAssembly module built by scripts/build.sh. The module starts itself on the
// canvas with id "flare-canvas".
import init from "./pkg/flare_web.js";

init().catch((error) => {
  document.body.textContent = `Flare could not start: ${error}`;
});
