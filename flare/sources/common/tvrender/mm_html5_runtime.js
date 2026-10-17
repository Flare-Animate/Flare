// minnamate's HTML5 Canvas player. It plays `movie` (movie.js) on <canvas id="movie">:
// every timeline's frames come as display lists, so playing is moving each timeline's
// playhead, keeping movie clips in step with what their parents show, running the
// frame scripts of the frames entered, and drawing.
(function () {
  "use strict";

  var canvas = document.getElementById("movie");
  var ctx = canvas.getContext("2d");
  var ratio = window.devicePixelRatio || 1;
  // No colour change: the multipliers, then the offsets.
  var PLAIN = [1, 1, 1, 1, 0, 0, 0, 0];
  // Timelines whose frame scripts are due.
  var due = [];
  // The bitmaps; one still loading draws nothing.
  var images = movie.images.map(function (b) {
    var img = new Image();
    img.src = b.src;
    return img;
  });

  function MovieClip(symbol, parent, button) {
    this.symbol = symbol;
    this.timeline = movie.timelines[symbol];
    this.parent = parent;
    this.name = null;
    this.currentFrame = 0;
    this.totalFrames = this.timeline.frames.length;
    this.button = !!button;
    this.playing = !button;
    this.children = {};
    if (!button) enter(this);
  }

  var clip = MovieClip.prototype;
  clip.stop = function () { this.playing = false; };
  clip.play = function () { this.playing = !this.button; };
  clip.gotoAndPlay = function (frame, scene) { this.go(this.frameOf(frame, scene), true); };
  clip.gotoAndStop = function (frame, scene) { this.go(this.frameOf(frame, scene), false); };
  clip.nextFrame = function () { this.go(Math.min(this.currentFrame + 1, this.totalFrames - 1), false); };
  clip.prevFrame = function () { this.go(Math.max(this.currentFrame - 1, 0), false); };
  clip.nextScene = function () {
    var scenes = this.scenes(), i = scenes.indexOf(this.scene());
    if (i + 1 < scenes.length) this.go(scenes[i + 1][1], true);
  };
  clip.prevScene = function () {
    var scenes = this.scenes(), i = scenes.indexOf(this.scene());
    if (i > 0) this.go(scenes[i - 1][1], true);
  };
  clip.getChildByName = function (name) {
    for (var key in this.children) if (this.children[key].name === name) return this.children[key];
    return null;
  };

  // [name, first frame, length] of each scene; a movie clip's timeline is one.
  clip.scenes = function () { return this.timeline.scenes || [["", 0, this.totalFrames]]; };
  clip.scene = function () {
    var scenes = this.scenes();
    for (var i = 0; i < scenes.length; i++) if (this.currentFrame < scenes[i][1] + scenes[i][2]) return scenes[i];
    return scenes[scenes.length - 1];
  };

  // Where a goto lands: a number counts from 0 in the scene; a label is looked for in the
  // scene first. null if there's no such scene or label.
  clip.frameOf = function (frame, scene) {
    var s = this.scene(), labels = this.timeline.labels, i;
    if (scene != null) {
      s = this.scenes().filter(function (t) { return t[0] === scene; })[0];
      if (!s) return null;
    }
    if (typeof frame === "number") return s[1] + Math.max(0, Math.min(Math.floor(frame), s[2] - 1));
    for (i = 0; i < labels.length; i++) if (labels[i][0] === frame && labels[i][1] >= s[1] && labels[i][1] < s[1] + s[2]) return labels[i][1];
    for (i = 0; scene == null && i < labels.length; i++) if (labels[i][0] === frame) return labels[i][1];
    return null;
  };

  clip.go = function (frame, play) {
    if (frame === null || this.button) return;
    this.playing = play;
    if (frame !== this.currentFrame) {
      this.currentFrame = frame;
      this.sync();
      enter(this);
    }
  };

  // The movie clips this frame shows: those still shown play on, new ones start, and those
  // gone run no more scripts.
  clip.sync = function () {
    var old = this.children, kept = {}, self = this, key;
    each(this.timeline.frames[this.currentFrame] || [], function (item) {
      var child = old[item.k];
      if (!child || child.symbol !== item.clip) child = new MovieClip(item.clip, self, item.b);
      child.name = item.n || null;
      kept[item.k] = child;
    });
    for (key in old) if (kept[key] !== old[key]) gone(old[key]);
    this.children = kept;
    for (key in kept) kept[key].sync();
  };

  function gone(clip) {
    clip.due = false;
    for (var key in clip.children) gone(clip.children[key]);
  }

  // Each clip of a display list, masks before what they mask.
  function each(items, visit) {
    items.forEach(function (item) {
      if (item.mask) {
        each(item.mask, visit);
        each(item.items, visit);
      } else if (item.clip !== undefined) {
        visit(item);
      }
    });
  }

  function enter(clip) {
    clip.due = true;
    due.push(clip);
  }

  // Every playing timeline moves on a frame, back to its first after its last.
  function advance(clip) {
    if (clip.playing && clip.totalFrames > 1) {
      clip.currentFrame = (clip.currentFrame + 1) % clip.totalFrames;
      enter(clip);
    }
    for (var key in clip.children) advance(clip.children[key]);
  }

  // Runs the scripts of the frames entered; a goto in one runs its frame's too.
  function runScripts() {
    for (var runs = 0; due.length && runs < 1000; runs++) {
      var clip = due.shift(), script = clip.timeline.scripts[clip.currentFrame];
      if (!clip.due) continue;
      clip.due = false;
      if (script) {
        try {
          script.call(clip);
        } catch (e) {
          console.error(e);
        }
      }
    }
    due.length = 0;
  }

  function draw() {
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.fillStyle = movie.background;
    ctx.fillRect(0, 0, movie.width, movie.height);
    drawItems(ctx, root.timeline.frames[root.currentFrame] || [], root, PLAIN, false);
  }

  // A display list drawn on `g`, colours changed by `cx`; `masking` draws fills only, opaque.
  function drawItems(g, items, clip, cx, masking) {
    items.forEach(function (item) {
      if (item.mask) return drawMasked(g, item, clip, cx, masking);
      // Visible unchecked: a clip plays on, undrawn.
      if (item.h) return;
      var c = item.c ? combine(cx, item.c) : cx, m = item.m;
      g.save();
      g.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
      if (item.s !== undefined) {
        drawShape(g, movie.shapes[item.s], c, masking);
      } else if (item.i !== undefined) {
        drawBitmap(g, item.i, c, masking);
      } else {
        var child = clip.children[item.k];
        if (child) drawItems(g, child.timeline.frames[child.currentFrame] || [], child, c, masking);
      }
      g.restore();
    });
  }

  // What a mask masks, on a layer of its own, kept where the mask's fills cover.
  // ponytail: two canvas-sized scratch canvases a mask a frame; keep them if masks get slow.
  function drawMasked(g, item, clip, cx, masking) {
    var layer = scratch(g), mask = scratch(g);
    drawItems(layer, item.items, clip, cx, masking);
    drawItems(mask, item.mask, clip, PLAIN, true);
    layer.setTransform(1, 0, 0, 1, 0, 0);
    layer.globalCompositeOperation = "destination-in";
    layer.drawImage(mask.canvas, 0, 0);
    g.save();
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.drawImage(layer.canvas, 0, 0);
    g.restore();
  }

  function scratch(g) {
    var c = document.createElement("canvas");
    c.width = g.canvas.width;
    c.height = g.canvas.height;
    var s = c.getContext("2d");
    s.setTransform(g.getTransform());
    return s;
  }

  function drawShape(g, shape, cx, masking) {
    shape.f.forEach(function (fill) {
      var path = fill.p || (fill.p = new Path2D(fill.d));
      if (fill.g && !masking) return drawGradient(g, path, fill.g, cx);
      var bitmap = fill.b && !masking;
      g.fillStyle = masking ? "#000" : bitmap ? pattern(g, fill.b) : css(fill.c, cx);
      g.globalAlpha = bitmap ? alpha(cx) : 1;
      g.fill(path, "evenodd");
      g.globalAlpha = 1;
    });
    // A mask's strokes don't count.
    if (masking) return;
    shape.s.forEach(function (stroke) {
      var path = stroke.p || (stroke.p = new Path2D(stroke.d));
      g.lineWidth = stroke.w || 0.05;
      g.lineCap = stroke.cap;
      g.lineJoin = stroke.join;
      g.miterLimit = 3;
      // ponytail: a gradient stroke draws in its first colour; its gradient needs the stroke's own space.
      g.strokeStyle = stroke.b ? pattern(g, stroke.b) : css(stroke.c || ((stroke.g && stroke.g.s[0]) || [0, [0, 0, 0, 1]])[1], cx);
      g.globalAlpha = stroke.b ? alpha(cx) : 1;
      g.stroke(path);
      g.globalAlpha = 1;
    });
  }

  // A bitmap from (0, 0) to its size; in a mask, its box.
  function drawBitmap(g, i, cx, masking) {
    var b = movie.images[i];
    if (masking) {
      g.fillStyle = "#000";
      return g.fillRect(0, 0, b.w, b.h);
    }
    g.globalAlpha = alpha(cx);
    if (images[i].complete) g.drawImage(images[i], 0, 0, b.w, b.h);
  }

  // A bitmap paint, its matrix taking the image's pixels to the shape's space: tiled, or once.
  // ponytail: past a clipped fill's edges its edge pixels don't go on as in the SWF; nothing does.
  function pattern(g, b) {
    var img = images[b.i];
    if (!img.complete || !img.naturalWidth) return "rgba(0,0,0,0)";
    var p = g.createPattern(img, b.r ? "repeat" : "no-repeat");
    p.setTransform(new DOMMatrix(b.m));
    return p;
  }

  // ponytail: a bitmap takes only its colour change's alpha multiplier; canvas has no per-channel one.
  function alpha(cx) {
    return Math.min(1, Math.max(0, cx[3]));
  }

  // A gradient fills its ±819.2 px square, placed by its matrix, inside the path.
  function drawGradient(g, path, grad, cx) {
    var m = grad.m, r = 819.2;
    g.save();
    g.clip(path, "evenodd");
    g.transform(m[0], m[1], m[2], m[3], m[4], m[5]);
    var paint = grad.t === "r" ? g.createRadialGradient(grad.f * r, 0, 0, 0, 0, r) : g.createLinearGradient(-r, 0, r, 0);
    grad.s.forEach(function (stop) { paint.addColorStop(Math.min(Math.max(stop[0], 0), 1), css(stop[1], cx)); });
    g.fillStyle = paint;
    g.fillRect(-1e5, -1e5, 2e5, 2e5);
    g.restore();
  }

  function css(c, cx) {
    var v = [0, 1, 2, 3].map(function (i) { return Math.min(1, Math.max(0, c[i] * cx[i] + cx[i + 4])); });
    return "rgba(" + Math.round(v[0] * 255) + "," + Math.round(v[1] * 255) + "," + Math.round(v[2] * 255) + "," + v[3] + ")";
  }

  // `parent` applied after `child`, as nested clips colour what they hold.
  function combine(parent, child) {
    var out = [];
    for (var i = 0; i < 4; i++) out[i] = child[i] * parent[i];
    for (i = 0; i < 4; i++) out[i + 4] = child[i + 4] * parent[i] + parent[i + 4];
    return out;
  }

  canvas.width = movie.width * ratio;
  canvas.height = movie.height * ratio;
  canvas.style.width = movie.width + "px";
  canvas.style.height = movie.height + "px";
  var root = new MovieClip("", null, false);
  window.exportRoot = root;
  root.sync();
  runScripts();
  draw();

  // One frame each 1/fps of a second; a page far behind drops frames rather than racing.
  var step = 1000 / movie.fps, last = performance.now(), wait = 0;
  function frame(now) {
    wait += now - last;
    last = now;
    var stepped = 0;
    for (; wait >= step && stepped < 4; stepped++) {
      wait -= step;
      advance(root);
      root.sync();
      runScripts();
    }
    if (wait > step) wait = 0;
    if (stepped) draw();
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
})();
