// The game in the page: the player's program compiled by rawr's universe
// (web/universe, built by web/build.sh) and run in a frame of its own.
//
// The universe's module is compiled once. Each run of the program needs an
// instance of its own, and the game's web shell keeps the instance it runs
// on, so each run gets a fresh frame that instantiates the module. One
// frame always waits, ready, under the one the player sees.
//
// The game finds the `elevato` global on its window (src/reload.rs): it
// tells the game whether the page restarted it for a save, and the error
// it started with when the saved program didn't compile. The game hands
// each save to the page; a save that compiles replaces the game with a
// new one running it, and one that doesn't goes back to the running game
// as its error.

const GLUE = new URL("universe.js", document.baseURI).href;
/** Where the game keeps the player's program (src/storage.rs). */
const DRAFT = "elevato_program_v3";

const loaded = Promise.all([
  WebAssembly.compileStreaming(fetch("universe_bg.wasm")),
  text("universe.toml"),
  text("starter.rs"),
]).then(([module, manifest, starter]) => ({ module, manifest, starter }));

async function text(path) {
  const response = await fetch(path);
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return response.text();
}

/**
 * The frame's document: it imports the glue, takes the compiled module from
 * the page, instantiates it, and hands the page the instance.
 */
const FRAME = `<!doctype html><html><head><meta charset="utf-8"><style>
/* The canvas's percentage height needs a definite chain above it, and
   touch-action hands every finger gesture to the game rather than to
   the browser's pan and zoom. */
html, body { margin: 0; height: 100%; overflow: hidden; overscroll-behavior: none; }
canvas { display: block; width: 100%; height: 100%; touch-action: none; }
:focus, :focus-visible { outline: none; }
</style></head><body><script type="module">
import * as glue from ${JSON.stringify(GLUE)};
const page = parent;
const consoleError = console.error.bind(console);
console.error = (...args) => {
  const message = args.map(String).join(" ");
  if (/panicked|wasm|RuntimeError|unreachable/i.test(message)) page.report("panic", message, "");
  consoleError(...args);
};
// Mouse back/forward are the reference's history keys; the browser must
// not navigate as well.
for (const kind of ["mousedown", "mouseup", "pointerup", "auxclick"]) {
  addEventListener(kind, (e) => { if (e.button === 3 || e.button === 4) e.preventDefault(); });
}
const say = (detail) => frameElement.dispatchEvent(new CustomEvent("universe", { detail }));
addEventListener("message", (event) => {
  glue.default({ module_or_path: event.data }).then(
    () => say({ instance: glue }),
    (error) => say({ error: String(error) }),
  );
}, { once: true });
say({ listening: true });
</script></body></html>`;

/** A new frame under the game, its instance ready to run. */
async function frame() {
  const { module } = await loaded;
  const element = document.createElement("iframe");
  element.className = "game next";
  element.title = "elevato.rs";
  const ready = new Promise((resolve, reject) => {
    element.addEventListener("universe", (event) => {
      const detail = event.detail;
      if (detail.listening) element.contentWindow.postMessage(module, "*");
      else if (detail.instance) resolve(detail.instance);
      else reject(new Error(detail.error));
    });
  });
  element.srcdoc = FRAME;
  document.body.prepend(element);
  const instance = await ready;
  instance.prepare();
  return { element, instance };
}

let next = frame();
/** The frame the player sees. */
let shown = null;

/**
 * Run `source` in the waiting frame, which tells the game `global`. The
 * answer is the frame when the program runs, else the error.
 */
async function run(source, global) {
  const { manifest } = await loaded;
  const ready = await next;
  next = frame();
  ready.element.contentWindow.elevato = global;
  let answer;
  try {
    answer = JSON.parse(ready.instance.run(manifest, source));
  } catch (error) {
    answer = { kind: "error", text: ready.instance.panic_message?.() ?? String(error) };
  }
  if (answer.kind === "text" && answer.status === 0) return { frame: ready };
  ready.element.remove();
  const text = answer.kind === "error"
    ? answer.text
    : answer.output.map((chunk) => chunk.text).join("");
  return { error: text };
}

/** Put `frame` on top, in place of the game it replaces. */
function show(frame) {
  frame.element.classList.remove("next");
  shown?.element.remove();
  shown = frame;
  document.getElementById("intro")?.remove();
  frame.element.focus();
}

async function boot() {
  const { starter } = await loaded;
  const draft = localStorage.getItem(DRAFT);
  let answer = await run(draft ?? starter, { restarted: false });
  // A saved program that doesn't compile: the starter runs, and the game
  // shows the draft and its error.
  if (answer.error !== undefined && draft !== null) {
    answer = await run(starter, { restarted: false, failed: answer.error });
  }
  if (answer.error !== undefined) throw new Error(answer.error);
  show(answer.frame);
}

/** Saves wait their turn, so the last one wins. */
let applying = Promise.resolve();

addEventListener("message", (event) => {
  if (!shown || event.source !== shown.element.contentWindow) return;
  if (event.data?.elevato !== "apply") return;
  const source = event.data.source;
  const from = shown;
  applying = applying.then(async () => {
    const answer = await run(source, { restarted: true });
    if (answer.frame) show(answer.frame);
    else from.element.contentWindow?.postMessage({ elevato: "failed", text: answer.error }, "*");
  });
});

boot().catch((error) => bootFailure(String(error.message ?? error)));
