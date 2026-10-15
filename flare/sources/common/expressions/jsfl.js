// JSFL compatibility: the common part of the authoring tool's JavaScript API
// (fl / Document / Timeline / Layer / Frame / Library / Element) mapped onto
// Flare commands, so existing .jsfl scripts run here. Frames and
// layers are 0-based, as in JSFL. Unsupported calls throw a clear error.
var fl = (function () {
  "use strict";
  var fill = "#0000FF", stroke = "#000000", strokeSize = 1, selectedItem = null;
  function uriPath(u) {
    u = String(u);
    if (u.indexOf("file:///") === 0) {
      u = decodeURIComponent(u.slice(7));
      if (/^\/[A-Za-z]\|/.test(u)) u = u.slice(1, 2) + ":" + u.slice(3); // file:///C|/x → C:/x
    }
    return u;
  }
  function unsupported(name) { return function () { throw new Error("JSFL: " + name + " is not supported by Flare yet"); }; }
  function color(c) {
    if (c === null || c === undefined) return null;
    if (typeof c === "number") return "#" + ("000000" + c.toString(16)).slice(-6);
    c = String(c);
    if (/^#[0-9a-f]{8}$/i.test(c)) return "#" + c.slice(1, 7) + c.slice(7, 9);
    return c;
  }
  function kindName(k) { return k === "movieclip" ? "movie clip" : k; }
  function symKind(t) {
    t = String(t || "graphic").toLowerCase().replace(/\s|_/g, "");
    return t === "movieclip" ? "movieclip" : t === "button" ? "button" : "graphic";
  }
  function info() { return ac.doc.info(); }
  function layerNum(i) { return (i === undefined || i === null ? tl.currentLayer : i) + 1; }

  // ---------------------------------------------------------------- elements
  function element(e) {
    var o = {
      elementType: e.kind === "merge_shape" ? "shape" : e.kind === "bitmap" ? "instance" : e.kind,
      instanceType: e.kind === "instance" ? "symbol" : e.kind === "bitmap" ? "bitmap" : undefined,
      libraryItem: e.props && (e.props.symbol || e.props.bitmap) ? { name: e.props.symbol || e.props.bitmap } : undefined,
      layer: null,
      depth: e.index,
      _l: e.layer, _i: e.index,
    };
    function only(fn) {
      var keep = ac.selection.info().map(function (s) { return [s.layer, s.index]; });
      ac.select.element({ layer: o._l, index: o._i });
      try { return fn(); } finally {
        ac.select.none();
        keep.forEach(function (k) { ac.select.element({ layer: k[0], index: k[1], add: true }); });
      }
    }
    function cur() { return ac.frame.info({ layer: o._l }).elements[o._i]; }
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return cur().name; }, set: function (v) { only(function () { ac.selection.set({ name: String(v) }); }); } });
    Object.defineProperty(o, "x", { enumerable: true, get: function () { return cur().x; }, set: function (v) { only(function () { ac.selection.set_position({ x: +v, y: cur().y }); }); } });
    Object.defineProperty(o, "y", { enumerable: true, get: function () { return cur().y; }, set: function (v) { only(function () { ac.selection.set_position({ x: cur().x, y: +v }); }); } });
    Object.defineProperty(o, "width", { enumerable: true, get: function () { return cur().width; }, set: function (v) { only(function () { var c = cur(); ac.selection.scale({ sx: +v / c.width, sy: 1 }); }); } });
    Object.defineProperty(o, "height", { enumerable: true, get: function () { return cur().height; }, set: function (v) { only(function () { var c = cur(); ac.selection.scale({ sx: 1, sy: +v / c.height }); }); } });
    if (e.kind === "text") {
      Object.defineProperty(o, "text", { enumerable: true, get: function () { return cur().props.text; }, set: function (v) { only(function () { ac.selection.set({ text: String(v) }); }); } });
      o.getTextString = function () { return cur().props.text; };
      o.setTextString = function (v) { o.text = v; };
    }
    return o;
  }

  // ---------------------------------------------------------------- frames & layers
  function frameObj(li, f) {
    var fi = ac.frame.info({ layer: li + 1, frame: f + 1 }) || { start: f + 1, duration: 1, elements: [] };
    var o = { startFrame: fi.start - 1, duration: fi.duration, elements: fi.elements.map(element) };
    function at(fn) { var k = tl.currentFrame, l = tl.currentLayer; try { return fn(); } finally { ac.timeline.goto({ frame: k + 1 }); ac.layer.select({ layer: l + 1 }); } }
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return fi.label || ""; }, set: function (v) { at(function () { ac.timeline.set_label({ label: String(v), layer: li + 1, frame: fi.start }); }); fi.label = v; } });
    Object.defineProperty(o, "labelType", { enumerable: true, get: function () { return fi.label ? "name" : "none"; }, set: function () {} });
    Object.defineProperty(o, "tweenType", { enumerable: true, get: function () { return fi.tween === "classic" ? "motion" : fi.tween === "motion" ? "motion object" : fi.tween; }, set: function (v) { at(function () { ac.timeline.set_tween({ kind: v === "motion" ? "classic" : v === "motion object" ? "motion" : v, layer: li + 1, frame: fi.start }); }); fi.tween = v; } });
    Object.defineProperty(o, "tweenEasing", { enumerable: true, get: function () { return fi.ease; }, set: function (v) { at(function () { ac.timeline.set_tween({ kind: fi.tween === "none" ? "classic" : fi.tween, ease: +v, layer: li + 1, frame: fi.start }); }); } });
    Object.defineProperty(o, "actionScript", { enumerable: true, get: function () { return fi.script || ""; }, set: function (v) { at(function () { ac.timeline.set_script({ script: String(v), layer: li + 1, frame: fi.start }); }); fi.script = v; } });
    Object.defineProperty(o, "soundName", { enumerable: true, get: function () { return fi.sound || ""; }, set: function (v) { at(function () { ac.timeline.set_sound({ sound: v || null, layer: li + 1, frame: fi.start }); }); } });
    o.isEmpty = fi.elements.length === 0;
    return o;
  }
  function layerObj(li) {
    var L = function () { return ac.layer.list()[li]; };
    var o = { _i: li };
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return L().name; }, set: function (v) { ac.layer.set({ layer: li + 1, name: String(v) }); } });
    Object.defineProperty(o, "visible", { enumerable: true, get: function () { return L().visible; }, set: function (v) { ac.layer.set({ layer: li + 1, visible: !!v }); } });
    Object.defineProperty(o, "locked", { enumerable: true, get: function () { return L().locked; }, set: function (v) { ac.layer.set({ layer: li + 1, locked: !!v }); } });
    Object.defineProperty(o, "outline", { enumerable: true, get: function () { return L().outline; }, set: function (v) { ac.layer.set({ layer: li + 1, outline: !!v }); } });
    Object.defineProperty(o, "color", { enumerable: true, get: function () { return L().color; }, set: function () {} });
    Object.defineProperty(o, "layerType", { enumerable: true, get: function () { var k = L().kind; return k === "mask" ? "mask" : k === "guide" ? "guide" : k === "folder" ? "folder" : L().parent ? "masked" : "normal"; }, set: function (v) { ac.layer.set({ layer: li + 1, kind: v === "masked" || v === "guided" ? "normal" : v }); } });
    Object.defineProperty(o, "parentLayer", { enumerable: true, get: function () { var p = L().parent; return p ? layerObj(p - 1) : null; }, set: function (v) { ac.layer.set({ layer: li + 1, parent: v ? v._i + 1 : 0 }); } });
    Object.defineProperty(o, "frameCount", { enumerable: true, get: function () { return L().frames; } });
    Object.defineProperty(o, "frames", { enumerable: true, get: function () {
      var n = L().frames, out = [];
      for (var f = 0; f < n; f++) out.push(frameObj(li, f));
      return out;
    } });
    return o;
  }

  // ---------------------------------------------------------------- timeline
  var tl = {
    get name() { return info().scene; },
    get layerCount() { return ac.layer.list().length; },
    get frameCount() { return info().frames; },
    get currentFrame() { return info().current_frame - 1; },
    set currentFrame(f) { ac.timeline.goto({ frame: f + 1 }); },
    get currentLayer() { return info().current_layer - 1; },
    set currentLayer(i) { ac.layer.select({ layer: i + 1 }); },
    get layers() { return ac.layer.list().map(function (_, i) { return layerObj(i); }); },
    addNewLayer: function (name, layerType, addAbove) {
      ac.layer.add({ name: name || undefined });
      var i = tl.currentLayer;
      if (layerType && layerType !== "normal") ac.layer.set({ layer: i + 1, kind: layerType });
      return i;
    },
    addLayer: function (l) { return tl.addNewLayer(l && l.name); },
    deleteLayer: function (i) { ac.layer.delete({ layer: layerNum(i) }); },
    setSelectedLayers: function (i) { ac.layer.select({ layer: (Array.isArray(i) ? i[0] : i) + 1 }); },
    getSelectedLayers: function () { return [tl.currentLayer]; },
    findLayerIndex: function (name) { var r = []; ac.layer.list().forEach(function (l, i) { if (l.name === name) r.push(i); }); return r.length ? r : undefined; },
    setSelectedFrames: function (a, b) {
      if (Array.isArray(a)) ac.timeline.select({ layer: a[0] + 1, frame: a[1] + 1, to_frame: a[2] });
      else ac.timeline.select({ layer: tl.currentLayer + 1, frame: a + 1, to_frame: (b === undefined ? a + 1 : b) });
    },
    insertFrames: function (n, all, at) {
      var args = { frames: n || 1 };
      if (at !== undefined) args.frame = at + 1;
      if (all) { var c = ac.layer.list().length; for (var i = 1; i <= c; i++) ac.timeline.insert_frames(Object.assign({ layer: i }, args)); }
      else ac.timeline.insert_frames(args);
    },
    removeFrames: function (a, b) { ac.timeline.remove_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    insertKeyframe: function (f) { ac.timeline.insert_keyframe({ frame: (f === undefined ? tl.currentFrame : f) + 1 }); },
    insertBlankKeyframe: function (f) { ac.timeline.insert_blank_keyframe({ frame: (f === undefined ? tl.currentFrame : f) + 1 }); },
    convertToKeyframes: function (a, b) { for (var f = a; f < (b === undefined ? a + 1 : b); f++) ac.timeline.insert_keyframe({ frame: f + 1 }); },
    convertToBlankKeyframes: function (a, b) { for (var f = a; f < (b === undefined ? a + 1 : b); f++) ac.timeline.insert_blank_keyframe({ frame: f + 1 }); },
    clearKeyframes: function (a, b) { for (var f = a; f < (b === undefined ? a + 1 : b); f++) { try { ac.timeline.clear_keyframe({ frame: f + 1 }); } catch (e) { /* not a keyframe */ } } },
    clearFrames: function (a, b) { ac.timeline.clear_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    reverseFrames: function (a, b) { ac.timeline.reverse_frames({ frame: a + 1, to_frame: b === undefined ? a + 1 : b }); },
    createMotionTween: function (a) { ac.timeline.set_tween({ kind: "classic", frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    createMotionObject: function (a) { ac.timeline.set_tween({ kind: "motion", frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    setFrameProperty: function (prop, value, a, b) {
      var from = a === undefined ? tl.currentFrame : a, to = b === undefined ? from + 1 : b;
      var spans = {};
      for (var f = from; f < to; f++) { var fi = ac.frame.info({ frame: f + 1 }); if (fi) spans[fi.start] = 1; }
      Object.keys(spans).forEach(function (s) { frameObj(tl.currentLayer, +s - 1)[prop === "easing" ? "tweenEasing" : prop] = value; });
    },
    getFrameProperty: function (prop, f) { return frameObj(tl.currentLayer, f === undefined ? tl.currentFrame : f)[prop === "easing" ? "tweenEasing" : prop]; },
    setLayerProperty: function (prop, value, which) {
      var targets = which === "all" ? ac.layer.list().map(function (_, i) { return i; }) : [tl.currentLayer];
      targets.forEach(function (i) { layerObj(i)[prop] = value; });
    },
    getLayerProperty: function (prop) { return layerObj(tl.currentLayer)[prop]; },
    copyFrames: function (a, b) { ac.timeline.copy_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    cutFrames: function (a, b) { ac.timeline.cut_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    pasteFrames: function (a) { ac.timeline.paste_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    copyLayers: unsupported("Timeline.copyLayers"),
    duplicateLayers: function () { throw new Error("JSFL: Timeline.duplicateLayers is not supported by Flare yet"); },
    expandFolder: function (expand, recurse, i) { if (i !== undefined) ac.layer.set({ layer: i + 1 }); },
  };

  // ---------------------------------------------------------------- library
  var library = {
    get items() {
      var out = ac.library.list().map(function (s) { return { name: s.name, itemType: kindName(s.kind.toLowerCase()), symbolType: kindName(s.kind.toLowerCase()), timeline: null, linkageClassName: "" }; });
      ac.sound.list().forEach(function (s) { out.push({ name: s.name, itemType: "sound" }); });
      return out;
    },
    itemExists: function (n) { return library.items.some(function (i) { return i.name === n; }); },
    findItemIndex: function (n) { var r = []; library.items.forEach(function (i, k) { if (i.name === n) r.push(k); }); return r; },
    selectItem: function (n) { selectedItem = n; return library.itemExists(n); },
    selectNone: function () { selectedItem = null; },
    getSelectedItems: function () { return library.items.filter(function (i) { return i.name === selectedItem; }); },
    addItemToDocument: function (pos, name) { ac.library.place({ symbol: name || selectedItem, x: pos ? pos.x : 0, y: pos ? pos.y : 0 }); return true; },
    deleteItem: function (n) { n = n || selectedItem; if (ac.sound.list().some(function (s) { return s.name === n; })) ac.sound.delete({ sound: n }); else ac.library.delete({ symbol: n }); return true; },
    renameItem: function (newName) { ac.library.rename({ symbol: selectedItem, name: newName }); selectedItem = newName; return true; },
    editItem: function (n) { ac.symbol.edit({ symbol: n || selectedItem }); return true; },
    addNewItem: function (type, name) { ac.symbol.new({ name: name, kind: symKind(type) }); ac.symbol.exit(); return true; },
    duplicateItem: unsupported("library.duplicateItem"),
    newFolder: function () { return true; },
  };

  // ---------------------------------------------------------------- document
  function rectArgs(r) { return { x: Math.min(r.left, r.right), y: Math.min(r.top, r.bottom), w: Math.abs(r.right - r.left), h: Math.abs(r.bottom - r.top) }; }
  var doc = {
    get name() { return "Untitled"; },
    get width() { return info().width; }, set width(v) { ac.doc.set({ width: +v }); },
    get height() { return info().height; }, set height(v) { ac.doc.set({ height: +v }); },
    get frameRate() { return info().fps; }, set frameRate(v) { ac.doc.set({ fps: +v }); },
    get backgroundColor() { return info().background; }, set backgroundColor(v) { ac.doc.set({ background: color(v) }); },
    get currentTimeline() { return 0; },
    get timelines() { return [tl]; },
    get selection() { return ac.selection.info().map(element); },
    set selection(arr) { ac.select.none(); (arr || []).forEach(function (e) { ac.select.element({ layer: e._l, index: e._i, add: true }); }); },
    library: library,
    getTimeline: function () { return tl; },
    editScene: function (i) { ac.scene.goto({ scene: i + 1 }); },
    addNewScene: function (name) { ac.scene.add({ name: name }); return true; },
    exitEditMode: function () { ac.symbol.exit(); },
    enterEditMode: function () { var s = ac.selection.info()[0]; if (s && s.props.symbol) ac.symbol.edit({ symbol: s.props.symbol }); },
    selectAll: function () { ac.select.all(); },
    selectNone: function () { ac.select.none(); },
    deleteSelection: function () { ac.selection.delete(); },
    duplicateSelection: function () { ac.selection.duplicate(); },
    group: function () { ac.selection.group(); },
    unGroup: function () { ac.selection.ungroup(); },
    breakApart: function () { ac.selection.break_apart(); },
    arrange: function (how) { ac.selection.arrange({ front: /front|forward/i.test(how) }); },
    setFillColor: function (c) { fill = color(c); var s = ac.selection.info(); if (s.length) ac.selection.set({ fill: fill }); },
    setStrokeColor: function (c) { stroke = color(c); var s = ac.selection.info(); if (s.length) ac.selection.set({ stroke: stroke }); },
    setStrokeSize: function (w) { strokeSize = +w; var s = ac.selection.info(); if (s.length) ac.selection.set({ stroke_width: strokeSize }); },
    setStroke: function (c, w, style) { doc.setStrokeColor(c); doc.setStrokeSize(w); },
    getCustomFill: function () { return { style: fill ? "solid" : "noFill", color: fill }; },
    setCustomFill: function (f) { if (f) doc.setFillColor(f.style === "noFill" ? null : f.color); },
    getCustomStroke: function () { return { style: stroke ? "solid" : "noStroke", color: stroke, thickness: strokeSize }; },
    setCustomStroke: function (s) { if (s) { stroke = s.style === "noStroke" ? null : color(s.color); if (s.thickness !== undefined) strokeSize = +s.thickness; } },
    addNewRectangle: function (r, round, noFill, noStroke) { var a = rectArgs(r); a.radius = round || 0; a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; a.stroke_width = strokeSize; ac.draw.rect(a); },
    addNewPrimitiveRectangle: function (r, round, noFill, noStroke) { var a = rectArgs(r); a.radius = round || 0; a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; ac.draw.rect_primitive(a); },
    addNewOval: function (r, noFill, noStroke) { var a = rectArgs(r); a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; a.stroke_width = strokeSize; ac.draw.oval(a); },
    addNewPrimitiveOval: function (r, noFill, noStroke) { var a = rectArgs(r); a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; ac.draw.oval_primitive(a); },
    addNewLine: function (p1, p2) { ac.draw.line({ x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y, stroke: stroke || "#000000", stroke_width: strokeSize }); },
    addNewPolystar: function (cx, cy, r, sides, star) { ac.draw.polystar({ cx: cx, cy: cy, r: r, sides: sides || 5, star: !!star, fill: fill, stroke: stroke }); },
    addNewText: function (r, text) { ac.draw.text({ x: r.left, y: r.top, text: text === undefined ? "" : String(text), color: fill || "#000000" }); },
    moveSelectionBy: function (d) { ac.selection.move({ dx: d.x || 0, dy: d.y || 0 }); },
    scaleSelection: function (sx, sy) { ac.selection.scale({ sx: sx, sy: sy === undefined ? sx : sy }); },
    rotateSelection: function (deg) { ac.selection.rotate({ degrees: deg }); },
    skewSelection: function (h, v) { ac.selection.skew({ h: h || 0, v: v || 0 }); },
    align: function (mode, toStage) {
      var m = { left: "left", right: "right", top: "top", bottom: "bottom", "vertical center": "vcenter", "horizontal center": "hcenter" }[mode];
      if (!m) throw new Error("JSFL align: unknown mode " + mode);
      ac.selection.align({ op: m, to_stage: !!toStage });
    },
    distribute: function (mode, toStage) {
      var m = { left: "dist_left", right: "dist_right", top: "dist_top", bottom: "dist_bottom", "vertical center": "dist_vcenter", "horizontal center": "dist_hcenter" }[mode];
      ac.selection.align({ op: m, to_stage: !!toStage });
    },
    match: function (w, h, toStage) { ac.selection.align({ op: w && h ? "match_size" : w ? "match_width" : "match_height", to_stage: !!toStage }); },
    convertToSymbol: function (type, name) { ac.selection.to_symbol({ name: name, kind: symKind(type) }); return { name: name, itemType: type }; },
    swapElement: function (name) { ac.selection.swap_symbol({ symbol: name }); },
    setElementProperty: function (prop, value) {
      if (prop === "name") ac.selection.set({ name: String(value) });
      else if (prop === "x" || prop === "y") { var s = ac.selection.info()[0]; ac.selection.set_position({ x: prop === "x" ? +value : s.x, y: prop === "y" ? +value : s.y }); }
      else throw new Error("JSFL setElementProperty: '" + prop + "' is not supported");
    },
    setInstanceAlpha: function (a) { ac.selection.color_effect({ style: "alpha", amount: +a }); },
    setInstanceBrightness: function (b) { ac.selection.color_effect({ style: "brightness", amount: +b }); },
    setInstanceTint: function (c, amount) { ac.selection.color_effect({ style: "tint", color: color(c), amount: +amount }); },
    setBlendMode: function (m) { ac.selection.set_blend({ mode: String(m) }); },
    addFilter: function (name) { ac.selection.add_filter({ type: String(name).replace(/Filter$/i, "").replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase() }); },
    removeAllFilters: function () { ac.selection.clear_filters(); },
    importFile: function (uri, toLibrary) {
      var p = uriPath(uri);
      if (/\.(wav|mp3|ogg|oga|flac)$/i.test(p)) return ac.import.sound({ path: p }) !== undefined;
      return ac.import.image({ path: p, place: !toLibrary }) !== undefined;
    },
    exportPNG: function (uri) { ac.export.png({ path: uriPath(uri) }); return true; },
    exportSVG: function (uri) { ac.export.svg({ path: uriPath(uri) }); return true; },
    save: function () { throw new Error("JSFL: use document.saveAsCopy(fileURI) (or File ▸ Save)"); },
    saveAsCopy: function (uri) { ac.doc.save({ path: uriPath(uri) }); return true; },
    getSelectionRect: function () {
      var s = ac.selection.info();
      if (!s.length) return { left: 0, top: 0, right: 0, bottom: 0 };
      var l = Math.min.apply(null, s.map(function (e) { return e.x; })), t = Math.min.apply(null, s.map(function (e) { return e.y; }));
      var r = Math.max.apply(null, s.map(function (e) { return e.x + e.width; })), b = Math.max.apply(null, s.map(function (e) { return e.y + e.height; }));
      return { left: l, top: t, right: r, bottom: b };
    },
    publish: unsupported("document.publish"),
    testMovie: unsupported("document.testMovie"),
  };

  var out = {
    version: "Flare JSFL shim 1.0",
    get documents() { return [doc]; },
    getDocumentDOM: function () { return doc; },
    createDocument: function () { return doc; },
    trace: function () { print.apply(null, arguments); },
    outputPanel: { trace: function (s) { print(s); }, clear: function () {} },
    browseForFileURL: function () { return null; },
    browseForFolderURL: function () { return null; },
    getAppMemoryInfo: function () { return 0; },
    runScript: unsupported("fl.runScript"),
    openDocument: unsupported("fl.openDocument"),
    closeDocument: function () {},
    get tools() { return { activeTool: null }; },
  };
  return out;
})();
var alert = function (m) { fl.trace(m); };
var confirm = function () { return true; };
var prompt = function (_, d) { return d === undefined ? null : d; };
var FLfile = (function () {
  function p(u) {
    u = String(u);
    if (u.indexOf("file:///") === 0) {
      u = decodeURIComponent(u.slice(7));
      if (/^\/[A-Za-z]\|/.test(u)) u = u.slice(1, 2) + ":" + u.slice(3);
    }
    return u;
  }
  function attempt(fn) { try { return fn(); } catch (e) { return false; } }
  return {
    uriToPlatformPath: p,
    platformPathToURI: function (q) { q = String(q).replace(/\\/g, "/"); return "file://" + (q.charAt(0) === "/" ? "" : "/") + encodeURI(q); },
    exists: function (u) { return ac.file.exists({ path: p(u) }); },
    read: function (u) { return attempt(function () { return ac.file.read({ path: p(u) }); }) || undefined; },
    write: function (u, text, mode) { return attempt(function () { return ac.file.write({ path: p(u), text: String(text), append: mode === "append" }); }); },
    createFolder: function (u) { return attempt(function () { return ac.file.mkdir({ path: p(u) }); }); },
    listFolder: function (u, kind) { return attempt(function () { return ac.file.list({ path: p(u).replace(/\/\*.*$/, ""), kind: kind || "all" }); }) || []; },
    remove: function () { throw new Error("JSFL: FLfile.remove is not supported (Flare scripts don't delete files)"); },
  };
})();
