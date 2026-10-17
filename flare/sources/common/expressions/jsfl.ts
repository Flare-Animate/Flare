// JSFL compatibility: the common part of the authoring tool's JavaScript API
// (fl / Document / Timeline / Layer / Frame / Library / Element) mapped onto
// Flare commands, so existing .jsfl scripts run here. Frames and
// layers are 0-based, as in JSFL. Unsupported calls throw a clear error.
//
// Typing note: JSFL values and the `ac` bridge are untyped by design, so the
// data-carrying parameters and holder objects are `any`. Control flow and
// the public names are unchanged from the original script.
var fl = (function () {
  "use strict";
  var fill: any = "#0000FF", stroke: any = "#000000", strokeSize: any = 1, selectedItem: any = null;
  function uriPath(u: any): any {
    u = String(u);
    if (u.indexOf("file:///") === 0) {
      u = decodeURIComponent(u.slice(7));
      if (/^\/[A-Za-z]\|/.test(u)) u = u.slice(1, 2) + ":" + u.slice(3); // file:///C|/x → C:/x
    }
    return u;
  }
  function unsupported(name: string): () => never { return function (): never { throw new Error("JSFL: " + name + " is not supported by Flare yet"); }; }
  function color(c: any): any {
    if (c === null || c === undefined) return null;
    if (typeof c === "number") return "#" + ("000000" + c.toString(16)).slice(-6);
    c = String(c);
    if (/^#[0-9a-f]{8}$/i.test(c)) return "#" + c.slice(1, 7) + c.slice(7, 9);
    return c;
  }
  function kindName(k: string): string { return k === "movieclip" ? "movie clip" : k; }
  function symKind(t: any): string {
    t = String(t || "graphic").toLowerCase().replace(/\s|_/g, "");
    return t === "movieclip" ? "movieclip" : t === "button" ? "button" : "graphic";
  }
  function info(): any { return ac.doc.info(); }
  function layerRange(a: number | null | undefined, b?: number | null): any {
    if (a === undefined || a === null) return {};
    var r: any = { layer: a + 1 };
    if (b !== undefined && b !== null) r.to_layer = b + 1;
    return r;
  }
  function layerNum(i: number | null | undefined): number { return (i === undefined || i === null ? tl.currentLayer : i) + 1; }

  // ---------------------------------------------------------------- elements
  function element(e: any): any {
    var o: any = {
      elementType: e.kind === "merge_shape" ? "shape" : e.kind === "bitmap" ? "instance" : e.kind,
      instanceType: e.kind === "instance" ? "symbol" : e.kind === "bitmap" ? "bitmap" : undefined,
      libraryItem: e.props && (e.props.symbol || e.props.bitmap) ? { name: e.props.symbol || e.props.bitmap } : undefined,
      layer: null,
      depth: e.index,
      _l: e.layer, _i: e.index,
    };
    function only(fn: () => any): any {
      var keep = ac.selection.info().map(function (s: any) { return [s.layer, s.index]; });
      ac.select.element({ layer: o._l, index: o._i });
      try { return fn(); } finally {
        ac.select.none();
        keep.forEach(function (k: any) { ac.select.element({ layer: k[0], index: k[1], add: true }); });
      }
    }
    function cur(): any { return ac.frame.info({ layer: o._l }).elements[o._i]; }
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return cur().name; }, set: function (v: any) { only(function () { ac.selection.set({ name: String(v) }); }); } });
    Object.defineProperty(o, "x", { enumerable: true, get: function () { return cur().x; }, set: function (v: any) { only(function () { ac.selection.set_position({ x: +v, y: cur().y }); }); } });
    Object.defineProperty(o, "y", { enumerable: true, get: function () { return cur().y; }, set: function (v: any) { only(function () { ac.selection.set_position({ x: cur().x, y: +v }); }); } });
    Object.defineProperty(o, "width", { enumerable: true, get: function () { return cur().width; }, set: function (v: any) { only(function () { var c = cur(); ac.selection.scale({ sx: +v / c.width, sy: 1 }); }); } });
    Object.defineProperty(o, "height", { enumerable: true, get: function () { return cur().height; }, set: function (v: any) { only(function () { var c = cur(); ac.selection.scale({ sx: 1, sy: +v / c.height }); }); } });
    if (e.kind === "text") {
      Object.defineProperty(o, "text", { enumerable: true, get: function () { return cur().props.text; }, set: function (v: any) { only(function () { ac.selection.set({ text: String(v) }); }); } });
      o.getTextString = function () { return cur().props.text; };
      o.setTextString = function (v: any) { o.text = v; };
    }
    return o;
  }

  // ---------------------------------------------------------------- frames & layers
  function frameObj(li: number, f: number): any {
    var fi: any = ac.frame.info({ layer: li + 1, frame: f + 1 }) || { start: f + 1, duration: 1, elements: [] };
    var o: any = { startFrame: fi.start - 1, duration: fi.duration, elements: fi.elements.map(element) };
    function at(fn: () => any): any { var k = tl.currentFrame, l = tl.currentLayer; try { return fn(); } finally { ac.timeline.goto({ frame: k + 1 }); ac.layer.select({ layer: l + 1 }); } }
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return fi.label || ""; }, set: function (v: any) { at(function () { ac.timeline.set_label({ label: String(v), layer: li + 1, frame: fi.start }); }); fi.label = v; } });
    Object.defineProperty(o, "labelType", { enumerable: true, get: function () { return fi.label ? "name" : "none"; }, set: function () {} });
    Object.defineProperty(o, "tweenType", { enumerable: true, get: function () { return fi.tween === "classic" ? "motion" : fi.tween === "motion" ? "motion object" : fi.tween; }, set: function (v: any) { at(function () { ac.timeline.set_tween({ kind: v === "motion" ? "classic" : v === "motion object" ? "motion" : v, layer: li + 1, frame: fi.start }); }); fi.tween = v; } });
    Object.defineProperty(o, "tweenEasing", { enumerable: true, get: function () { return fi.ease; }, set: function (v: any) { at(function () { ac.timeline.set_tween({ kind: fi.tween === "none" ? "classic" : fi.tween, ease: +v, layer: li + 1, frame: fi.start }); }); } });
    Object.defineProperty(o, "actionScript", { enumerable: true, get: function () { return fi.script || ""; }, set: function (v: any) { at(function () { ac.timeline.set_script({ script: String(v), layer: li + 1, frame: fi.start }); }); fi.script = v; } });
    Object.defineProperty(o, "soundName", { enumerable: true, get: function () { return fi.sound || ""; }, set: function (v: any) { at(function () { ac.timeline.set_sound({ sound: v || null, layer: li + 1, frame: fi.start }); }); } });
    o.isEmpty = fi.elements.length === 0;
    return o;
  }
  function layerObj(li: number): any {
    var L = function (): any { return ac.layer.list()[li]; };
    var o: any = { _i: li };
    Object.defineProperty(o, "name", { enumerable: true, get: function () { return L().name; }, set: function (v: any) { ac.layer.set({ layer: li + 1, name: String(v) }); } });
    Object.defineProperty(o, "visible", { enumerable: true, get: function () { return L().visible; }, set: function (v: any) { ac.layer.set({ layer: li + 1, visible: !!v }); } });
    Object.defineProperty(o, "locked", { enumerable: true, get: function () { return L().locked; }, set: function (v: any) { ac.layer.set({ layer: li + 1, locked: !!v }); } });
    Object.defineProperty(o, "outline", { enumerable: true, get: function () { return L().outline; }, set: function (v: any) { ac.layer.set({ layer: li + 1, outline: !!v }); } });
    Object.defineProperty(o, "color", { enumerable: true, get: function () { return L().color; }, set: function () {} });
    Object.defineProperty(o, "layerType", { enumerable: true, get: function () { var k = L().kind; return k === "mask" ? "mask" : k === "guide" ? "guide" : k === "folder" ? "folder" : L().parent ? "masked" : "normal"; }, set: function (v: any) { ac.layer.set({ layer: li + 1, kind: v === "masked" || v === "guided" ? "normal" : v }); } });
    Object.defineProperty(o, "parentLayer", { enumerable: true, get: function () { var p = L().parent; return p ? layerObj(p - 1) : null; }, set: function (v: any) { ac.layer.set({ layer: li + 1, parent: v ? v._i + 1 : 0 }); } });
    Object.defineProperty(o, "frameCount", { enumerable: true, get: function () { return L().frames; } });
    Object.defineProperty(o, "frames", { enumerable: true, get: function () {
      var n = L().frames, out: any[] = [];
      for (var f = 0; f < n; f++) out.push(frameObj(li, f));
      return out;
    } });
    return o;
  }

  // ---------------------------------------------------------------- timeline
  var tl: any = {
    get name(): any { return info().scene; },
    get layerCount(): number { return ac.layer.list().length; },
    get frameCount(): number { return info().frames; },
    get currentFrame(): number { return info().current_frame - 1; },
    set currentFrame(f: number) { ac.timeline.goto({ frame: f + 1 }); },
    get currentLayer(): number { return info().current_layer - 1; },
    set currentLayer(i: number) { ac.layer.select({ layer: i + 1 }); },
    get layers(): any[] { return ac.layer.list().map(function (_: any, i: number) { return layerObj(i); }); },
    addNewLayer: function (name?: any, layerType?: any, addAbove?: any): number {
      ac.layer.add({ name: name || undefined });
      var i = tl.currentLayer;
      if (layerType && layerType !== "normal") ac.layer.set({ layer: i + 1, kind: layerType });
      return i;
    },
    addLayer: function (l?: any): number { return tl.addNewLayer(l && l.name); },
    deleteLayer: function (i?: number): void { ac.layer.delete({ layer: layerNum(i) }); },
    setSelectedLayers: function (i: any): void { ac.layer.select({ layer: (Array.isArray(i) ? i[0] : i) + 1 }); },
    getSelectedLayers: function (): number[] { return [tl.currentLayer]; },
    findLayerIndex: function (name: any): number[] | undefined { var r: number[] = []; ac.layer.list().forEach(function (l: any, i: number) { if (l.name === name) r.push(i); }); return r.length ? r : undefined; },
    setSelectedFrames: function (a: any, b?: any): void {
      if (Array.isArray(a)) ac.timeline.select({ layer: a[0] + 1, frame: a[1] + 1, to_frame: a[2] });
      else ac.timeline.select({ layer: tl.currentLayer + 1, frame: a + 1, to_frame: (b === undefined ? a + 1 : b) });
    },
    insertFrames: function (n?: any, all?: any, at?: any): void {
      var args: any = { frames: n || 1 };
      if (at !== undefined) args.frame = at + 1;
      if (all) { var c = ac.layer.list().length; for (var i = 1; i <= c; i++) ac.timeline.insert_frames(Object.assign({ layer: i }, args)); }
      else ac.timeline.insert_frames(args);
    },
    removeFrames: function (a?: number, b?: number): void { ac.timeline.remove_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    insertKeyframe: function (f?: number): void { ac.timeline.insert_keyframe({ frame: (f === undefined ? tl.currentFrame : f) + 1 }); },
    insertBlankKeyframe: function (f?: number): void { ac.timeline.insert_blank_keyframe({ frame: (f === undefined ? tl.currentFrame : f) + 1 }); },
    convertToKeyframes: function (a: number, b?: number): void { for (var f = a; f < (b === undefined ? a + 1 : b); f++) ac.timeline.insert_keyframe({ frame: f + 1 }); },
    convertToBlankKeyframes: function (a: number, b?: number): void { for (var f = a; f < (b === undefined ? a + 1 : b); f++) ac.timeline.insert_blank_keyframe({ frame: f + 1 }); },
    clearKeyframes: function (a: number, b?: number): void { for (var f = a; f < (b === undefined ? a + 1 : b); f++) { try { ac.timeline.clear_keyframe({ frame: f + 1 }); } catch (e) { /* not a keyframe */ } } },
    clearFrames: function (a?: number, b?: number): void { ac.timeline.clear_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    reverseFrames: function (a?: number, b?: number): void { ac.timeline.reverse_frames(a === undefined ? {} : { frame: a + 1, to_frame: b === undefined ? a + 1 : b }); },
    createMotionTween: function (a?: number): void { ac.timeline.set_tween({ kind: "classic", frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    createMotionObject: function (a?: number): void { ac.timeline.set_tween({ kind: "motion", frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    setFrameProperty: function (prop: string, value: any, a?: number, b?: number): void {
      var from = a === undefined ? tl.currentFrame : a, to = b === undefined ? from + 1 : b;
      var spans: { [start: string]: number } = {};
      for (var f = from; f < to; f++) { var fi = ac.frame.info({ frame: f + 1 }); if (fi) spans[fi.start] = 1; }
      Object.keys(spans).forEach(function (s: string) { frameObj(tl.currentLayer, +s - 1)[prop === "easing" ? "tweenEasing" : prop] = value; });
    },
    getFrameProperty: function (prop: string, f?: number): any { return frameObj(tl.currentLayer, f === undefined ? tl.currentFrame : f)[prop === "easing" ? "tweenEasing" : prop]; },
    setLayerProperty: function (prop: string, value: any, which?: string): void {
      var targets: number[] = which === "all" ? ac.layer.list().map(function (_: any, i: number) { return i; }) : [tl.currentLayer];
      targets.forEach(function (i: number) { layerObj(i)[prop] = value; });
    },
    getLayerProperty: function (prop: string): any { return layerObj(tl.currentLayer)[prop]; },
    copyFrames: function (a?: number, b?: number): void { ac.timeline.copy_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    cutFrames: function (a?: number, b?: number): void { ac.timeline.cut_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1, to_frame: b === undefined ? undefined : b }); },
    pasteFrames: function (a?: number): void { ac.timeline.paste_frames({ frame: (a === undefined ? tl.currentFrame : a) + 1 }); },
    // Layer clipboard: Flare's Copy / Cut / Paste Layers commands. Indices are
    // 0-based and inclusive; none = the layers selected in the timeline.
    copyLayers: function (a?: number, b?: number): void { ac.layer.copy(layerRange(a, b)); },
    cutLayers: function (a?: number, b?: number): void { ac.layer.cut(layerRange(a, b)); },
    // pastes above the layer at `i` (default: the current layer), as the Paste Layers menu does
    pasteLayers: function (i?: number): void { ac.layer.paste(i === undefined ? {} : { layer: i + 1 }); },
    // duplicates the current layer (or the one at `i`) as "<name> copy"
    duplicateLayers: function (i?: number): void { ac.layer.duplicate(i === undefined ? {} : { layer: i + 1 }); },
    expandFolder: function (expand: any, recurse: any, i?: number): void { if (i !== undefined) ac.layer.set({ layer: i + 1 }); },
  };

  // ---------------------------------------------------------------- library
  var library: any = {
    get items(): any[] {
      var out: any[] = ac.library.list().map(function (s: any) { return { name: s.name, itemType: kindName(s.kind.toLowerCase()), symbolType: kindName(s.kind.toLowerCase()), timeline: null, linkageClassName: "" }; });
      ac.sound.list().forEach(function (s: any) { out.push({ name: s.name, itemType: "sound" }); });
      return out;
    },
    itemExists: function (n: any): boolean { return library.items.some(function (i: any) { return i.name === n; }); },
    findItemIndex: function (n: any): number[] { var r: number[] = []; library.items.forEach(function (i: any, k: number) { if (i.name === n) r.push(k); }); return r; },
    selectItem: function (n: any): boolean { selectedItem = n; return library.itemExists(n); },
    selectNone: function (): void { selectedItem = null; },
    getSelectedItems: function (): any[] { return library.items.filter(function (i: any) { return i.name === selectedItem; }); },
    addItemToDocument: function (pos: any, name?: any): boolean { ac.library.place({ symbol: name || selectedItem, x: pos ? pos.x : 0, y: pos ? pos.y : 0 }); return true; },
    deleteItem: function (n: any): boolean { n = n || selectedItem; if (ac.sound.list().some(function (s: any) { return s.name === n; })) ac.sound.delete({ sound: n }); else ac.library.delete({ symbol: n }); return true; },
    renameItem: function (newName: any): boolean { ac.library.rename({ symbol: selectedItem, name: newName }); selectedItem = newName; return true; },
    editItem: function (n?: any): boolean { ac.symbol.edit({ symbol: n || selectedItem }); return true; },
    addNewItem: function (type: any, name: any): boolean { ac.symbol.new({ name: name, kind: symKind(type) }); ac.symbol.exit(); return true; },
    duplicateItem: unsupported("library.duplicateItem"),
    newFolder: function (): boolean { return true; },
  };

  // ---------------------------------------------------------------- document
  function rectArgs(r: any): any { return { x: Math.min(r.left, r.right), y: Math.min(r.top, r.bottom), w: Math.abs(r.right - r.left), h: Math.abs(r.bottom - r.top) }; }
  var doc: any = {
    get name(): string { return "Untitled"; },
    get width(): any { return info().width; }, set width(v: any) { ac.doc.set({ width: +v }); },
    get height(): any { return info().height; }, set height(v: any) { ac.doc.set({ height: +v }); },
    get frameRate(): any { return info().fps; }, set frameRate(v: any) { ac.doc.set({ fps: +v }); },
    get backgroundColor(): any { return info().background; }, set backgroundColor(v: any) { ac.doc.set({ background: color(v) }); },
    get currentTimeline(): number { return 0; },
    get timelines(): any[] { return [tl]; },
    get selection(): any[] { return ac.selection.info().map(element); },
    set selection(arr: any) { ac.select.none(); (arr || []).forEach(function (e: any) { ac.select.element({ layer: e._l, index: e._i, add: true }); }); },
    library: library,
    getTimeline: function (): any { return tl; },
    editScene: function (i: number): void { ac.scene.goto({ scene: i + 1 }); },
    addNewScene: function (name: any): boolean { ac.scene.add({ name: name }); return true; },
    exitEditMode: function (): void { ac.symbol.exit(); },
    enterEditMode: function (): void { var s = ac.selection.info()[0]; if (s && s.props.symbol) ac.symbol.edit({ symbol: s.props.symbol }); },
    selectAll: function (): void { ac.select.all(); },
    selectNone: function (): void { ac.select.none(); },
    deleteSelection: function (): void { ac.selection.delete(); },
    duplicateSelection: function (): void { ac.selection.duplicate(); },
    group: function (): void { ac.selection.group(); },
    unGroup: function (): void { ac.selection.ungroup(); },
    breakApart: function (): void { ac.selection.break_apart(); },
    arrange: function (how: any): void { ac.selection.arrange({ front: /front|forward/i.test(how) }); },
    setFillColor: function (c: any): void { fill = color(c); var s = ac.selection.info(); if (s.length) ac.selection.set({ fill: fill }); },
    setStrokeColor: function (c: any): void { stroke = color(c); var s = ac.selection.info(); if (s.length) ac.selection.set({ stroke: stroke }); },
    setStrokeSize: function (w: any): void { strokeSize = +w; var s = ac.selection.info(); if (s.length) ac.selection.set({ stroke_width: strokeSize }); },
    setStroke: function (c: any, w: any, style?: any): void { doc.setStrokeColor(c); doc.setStrokeSize(w); },
    getCustomFill: function (): any { return { style: fill ? "solid" : "noFill", color: fill }; },
    setCustomFill: function (f: any): void { if (f) doc.setFillColor(f.style === "noFill" ? null : f.color); },
    getCustomStroke: function (): any { return { style: stroke ? "solid" : "noStroke", color: stroke, thickness: strokeSize }; },
    setCustomStroke: function (s: any): void { if (s) { stroke = s.style === "noStroke" ? null : color(s.color); if (s.thickness !== undefined) strokeSize = +s.thickness; } },
    addNewRectangle: function (r: any, round?: any, noFill?: any, noStroke?: any): void { var a = rectArgs(r); a.radius = round || 0; a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; a.stroke_width = strokeSize; ac.draw.rect(a); },
    addNewPrimitiveRectangle: function (r: any, round?: any, noFill?: any, noStroke?: any): void { var a = rectArgs(r); a.radius = round || 0; a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; ac.draw.rect_primitive(a); },
    addNewOval: function (r: any, noFill?: any, noStroke?: any): void { var a = rectArgs(r); a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; a.stroke_width = strokeSize; ac.draw.oval(a); },
    addNewPrimitiveOval: function (r: any, noFill?: any, noStroke?: any): void { var a = rectArgs(r); a.fill = noFill ? null : fill; a.stroke = noStroke ? null : stroke; ac.draw.oval_primitive(a); },
    addNewLine: function (p1: any, p2: any): void { ac.draw.line({ x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y, stroke: stroke || "#000000", stroke_width: strokeSize }); },
    addNewPolystar: function (cx: number, cy: number, r: number, sides?: number, star?: any): void { ac.draw.polystar({ cx: cx, cy: cy, r: r, sides: sides || 5, star: !!star, fill: fill, stroke: stroke }); },
    addNewText: function (r: any, text?: any): void { ac.draw.text({ x: r.left, y: r.top, text: text === undefined ? "" : String(text), color: fill || "#000000" }); },
    moveSelectionBy: function (d: any): void { ac.selection.move({ dx: d.x || 0, dy: d.y || 0 }); },
    scaleSelection: function (sx: number, sy?: number): void { ac.selection.scale({ sx: sx, sy: sy === undefined ? sx : sy }); },
    rotateSelection: function (deg: number): void { ac.selection.rotate({ degrees: deg }); },
    skewSelection: function (h?: number, v?: number): void { ac.selection.skew({ h: h || 0, v: v || 0 }); },
    align: function (mode: string, toStage?: any): void {
      var modes: { [mode: string]: string } = { left: "left", right: "right", top: "top", bottom: "bottom", "vertical center": "vcenter", "horizontal center": "hcenter" };
      var m = modes[mode];
      if (!m) throw new Error("JSFL align: unknown mode " + mode);
      ac.selection.align({ op: m, to_stage: !!toStage });
    },
    distribute: function (mode: string, toStage?: any): void {
      var modes: { [mode: string]: string } = { left: "dist_left", right: "dist_right", top: "dist_top", bottom: "dist_bottom", "vertical center": "dist_vcenter", "horizontal center": "dist_hcenter" };
      ac.selection.align({ op: modes[mode], to_stage: !!toStage });
    },
    match: function (w: any, h: any, toStage?: any): void { ac.selection.align({ op: w && h ? "match_size" : w ? "match_width" : "match_height", to_stage: !!toStage }); },
    convertToSymbol: function (type: any, name: any): any { ac.selection.to_symbol({ name: name, kind: symKind(type) }); return { name: name, itemType: type }; },
    swapElement: function (name: any): void { ac.selection.swap_symbol({ symbol: name }); },
    setElementProperty: function (prop: string, value: any): void {
      if (prop === "name") ac.selection.set({ name: String(value) });
      else if (prop === "x" || prop === "y") { var s = ac.selection.info()[0]; ac.selection.set_position({ x: prop === "x" ? +value : s.x, y: prop === "y" ? +value : s.y }); }
      else throw new Error("JSFL setElementProperty: '" + prop + "' is not supported");
    },
    setInstanceAlpha: function (a: any): void { ac.selection.color_effect({ style: "alpha", amount: +a }); },
    setInstanceBrightness: function (b: any): void { ac.selection.color_effect({ style: "brightness", amount: +b }); },
    setInstanceTint: function (c: any, amount: any): void { ac.selection.color_effect({ style: "tint", color: color(c), amount: +amount }); },
    setBlendMode: function (m: any): void { ac.selection.set_blend({ mode: String(m) }); },
    addFilter: function (name: any): void { ac.selection.add_filter({ type: String(name).replace(/Filter$/i, "").replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase() }); },
    removeAllFilters: function (): void { ac.selection.clear_filters(); },
    importFile: function (uri: any, toLibrary?: any): boolean {
      var p = uriPath(uri);
      if (/\.(wav|mp3|ogg|oga|flac)$/i.test(p)) return ac.import.sound({ path: p }) !== undefined;
      return ac.import.image({ path: p, place: !toLibrary }) !== undefined;
    },
    exportPNG: function (uri: any): boolean { ac.export.png({ path: uriPath(uri) }); return true; },
    exportSVG: function (uri: any): boolean { ac.export.svg({ path: uriPath(uri) }); return true; },
    save: function (): never { throw new Error("JSFL: use document.saveAsCopy(fileURI) (or File ▸ Save)"); },
    saveAsCopy: function (uri: any): boolean { ac.doc.save({ path: uriPath(uri) }); return true; },
    getSelectionRect: function (): any {
      var s = ac.selection.info();
      if (!s.length) return { left: 0, top: 0, right: 0, bottom: 0 };
      var l = Math.min.apply(null, s.map(function (e: any) { return e.x; })), t = Math.min.apply(null, s.map(function (e: any) { return e.y; }));
      var r = Math.max.apply(null, s.map(function (e: any) { return e.x + e.width; })), b = Math.max.apply(null, s.map(function (e: any) { return e.y + e.height; }));
      return { left: l, top: t, right: r, bottom: b };
    },
    publish: unsupported("document.publish"),
    testMovie: unsupported("document.testMovie"),
  };

  var out: any = {
    version: "Flare JSFL shim 1.0",
    get documents(): any[] { return [doc]; },
    getDocumentDOM: function (): any { return doc; },
    createDocument: function (): any { return doc; },
    trace: function (): void { print.apply(null, arguments as any); },
    outputPanel: { trace: function (s: any): void { print(s); }, clear: function (): void {} },
    browseForFileURL: function (): null { return null; },
    browseForFolderURL: function (): null { return null; },
    getAppMemoryInfo: function (): number { return 0; },
    runScript: unsupported("fl.runScript"),
    openDocument: unsupported("fl.openDocument"),
    closeDocument: function (): void {},
    get tools(): any { return { activeTool: null }; },
  };
  return out;
})();
var alert = function (m: any): void { fl.trace(m); };
var confirm = function (): boolean { return true; };
var prompt = function (_: any, d?: any): any { return d === undefined ? null : d; };
var FLfile = (function () {
  function p(u: any): any {
    u = String(u);
    if (u.indexOf("file:///") === 0) {
      u = decodeURIComponent(u.slice(7));
      if (/^\/[A-Za-z]\|/.test(u)) u = u.slice(1, 2) + ":" + u.slice(3);
    }
    return u;
  }
  function attempt(fn: () => any): any { try { return fn(); } catch (e) { return false; } }
  return {
    uriToPlatformPath: p,
    platformPathToURI: function (q: any): string { q = String(q).replace(/\\/g, "/"); return "file://" + (q.charAt(0) === "/" ? "" : "/") + encodeURI(q); },
    exists: function (u: any): any { return ac.file.exists({ path: p(u) }); },
    read: function (u: any): any { return attempt(function () { return ac.file.read({ path: p(u) }); }) || undefined; },
    write: function (u: any, text: any, mode?: any): any { return attempt(function () { return ac.file.write({ path: p(u), text: String(text), append: mode === "append" }); }); },
    createFolder: function (u: any): any { return attempt(function () { return ac.file.mkdir({ path: p(u) }); }); },
    listFolder: function (u: any, kind?: any): any { return attempt(function () { return ac.file.list({ path: p(u).replace(/\/\*.*$/, ""), kind: kind || "all" }); }) || []; },
    remove: function (): never { throw new Error("JSFL: FLfile.remove is not supported (Flare scripts don't delete files)"); },
  };
})();
