// Plays the animation baked into the page: each frame is a list of shapes with a matrix, drawn
// onto a canvas. Everything it needs is in the JSON in #flare-data.
(function () {
  "use strict";
  var data = JSON.parse(document.getElementById("flare-data").textContent);
  var canvas = document.getElementById("stage");
  var ctx = canvas.getContext("2d");
  var paths = data.shapes.map(function (shape) {
    return new Path2D(shape.d);
  });
  var scale = 1;
  var shown = 0;

  function css(color) {
    return "rgba(" + color[0] + "," + color[1] + "," + color[2] + "," + color[3] / 255 + ")";
  }

  function paintFor(shape) {
    if (shape.grad) {
      var g = shape.grad;
      var gradient = ctx.createLinearGradient(g[0], g[1], g[2], g[3]);
      gradient.addColorStop(0, css(g[4]));
      gradient.addColorStop(1, css(g[5]));
      return gradient;
    }
    return shape.fill ? css(shape.fill) : null;
  }

  // The region of a mask: the union of its shapes, each in its own place.
  function region(frame, index) {
    frame.regions = frame.regions || [];
    if (!frame.regions[index]) {
      var union = new Path2D();
      frame.clips[index].forEach(function (member) {
        union.addPath(paths[member[0]], new DOMMatrix(member.slice(1, 7)));
      });
      frame.regions[index] = union;
    }
    return frame.regions[index];
  }

  function draw(position) {
    shown = position;
    var frame = data.frames[data.seq[position]];
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = css(data.bg);
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    frame.shapes.forEach(function (item) {
      var shape = data.shapes[item[0]];
      ctx.save();
      (item[7] || []).forEach(function (clip) {
        ctx.setTransform(scale, 0, 0, scale, 0, 0);
        ctx.clip(region(frame, clip));
      });
      ctx.setTransform(
        scale * item[1], scale * item[2], scale * item[3],
        scale * item[4], scale * item[5], scale * item[6]
      );
      var paint = paintFor(shape);
      if (paint) {
        ctx.fillStyle = paint;
        ctx.fill(paths[item[0]]);
      }
      if (shape.stroke) {
        ctx.lineWidth = shape.stroke[0];
        ctx.strokeStyle = css(shape.stroke[1]);
        ctx.lineCap = shape.stroke[2];
        ctx.lineJoin = shape.stroke[3];
        ctx.stroke(paths[item[0]]);
      }
      ctx.restore();
    });
  }

  function fit() {
    var ratio = window.devicePixelRatio || 1;
    var zoom = 1;
    if (data.responsive) {
      zoom = Math.min(window.innerWidth / data.w, window.innerHeight / data.h);
    }
    canvas.style.width = data.w * zoom + "px";
    canvas.style.height = data.h * zoom + "px";
    canvas.width = Math.max(1, Math.round(data.w * zoom * ratio));
    canvas.height = Math.max(1, Math.round(data.h * zoom * ratio));
    scale = canvas.width / data.w;
    draw(shown);
  }

  // Sounds start at their cues' frames; a new pass through the timeline starts them over.
  var sounds = (data.sounds || []).map(function (sound) {
    var audio = new Audio("data:" + sound.mime + ";base64," + sound.data);
    audio.preload = "auto";
    return audio;
  });
  var cues = data.cues || [];
  var next = 0;
  var previous = -1;
  function stopSounds() {
    sounds.forEach(function (audio) {
      audio.pause();
      audio.currentTime = 0;
    });
  }
  function playCues(position) {
    if (position < previous) {
      stopSounds();
      next = 0;
    }
    previous = position;
    while (next < cues.length && cues[next][0] <= position) {
      var audio = sounds[cues[next][1]];
      next += 1;
      try {
        audio.currentTime = 0;
        var started = audio.play();
        if (started && started.catch) started.catch(function () {});
      } catch (error) {
        // A sound that will not play does not stop the picture.
      }
    }
  }

  var started = null;
  function tick(now) {
    if (started === null) started = now;
    var count = data.seq.length;
    var position = Math.floor(((now - started) * data.fps) / 1000);
    if (position >= count && !data.loop) {
      playCues(count - 1);
      draw(count - 1);
      return;
    }
    playCues(position % count);
    draw(position % count);
    window.requestAnimationFrame(tick);
  }

  function begin() {
    if (data.seq.length > 1 || cues.length > 0) {
      window.requestAnimationFrame(tick);
    }
  }

  window.addEventListener("resize", fit);
  fit();
  var start = document.getElementById("start");
  if (cues.length > 0 && start) {
    // Browsers keep a page silent until someone interacts with it.
    start.style.display = "flex";
    start.addEventListener("click", function () {
      start.style.display = "none";
      begin();
    });
  } else {
    begin();
  }
})();
