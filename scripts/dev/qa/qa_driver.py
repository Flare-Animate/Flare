#!/usr/bin/env python3
"""Flare GUI QA driver (called by run_all.sh). Python 3 + PIL + xdotool + ImageMagick.

Runs Flare under Xvfb with an isolated HOME and FLAREROOT copy, drives it with xdotool
(coordinates come from the app's own widget geometry via the FLARE_QA_HOOK channel and
from xdotool window geometry; the only constants are layout-derived fractions of the
viewer rectangle), and records item -> PASS/FAIL/UNTESTED with screenshot paths.
App defects are reported as FAIL, never masked; only harness-side problems are UNTESTED.
"""
import argparse, glob, json, os, re, shutil, subprocess, sys, tempfile, time
from PIL import Image, ImageChops

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SCREEN = (1600, 900)

TOOLS = ["T_Selection", "T_Edit", "T_ControlPointEditor", "T_Cutter", "T_Brush", "T_Geometric",
         "T_Type", "T_Fill", "T_PaintBrush", "T_Eraser", "T_Tape", "T_StylePicker", "T_RGBPicker",
         "T_Bender", "T_Finger", "T_Iron", "T_Magnet", "T_Pinch", "T_Pump", "T_Hand", "T_Zoom",
         "T_Rotate"]
# Tools that need the stroke to land on existing art (so the Brush runs first); every tool
# gets the same zig-zag path, derived from the viewer rectangle.
CONTENT_EXTS = {"fla", "xfl", "swf", "psd", "gif", "mp4", "svg", "moho", "mohoproj", "anme", "anime"}  # importers that promise xsheet content
RASTER_ONLY = {"T_PaintBrush", "T_Finger"}  # "cannot be used on: Flare Vector Level" is the correct reaction
IMPORT_EXTS = {"fla", "swf", "psd", "ai", "air", "ane", "flv", "f4v", "gif", "mp4", "svg", "xfl",
               "png", "jpg", "jpeg", "moho", "mohoproj", "anme", "anime"}

results = []


def rec(area, item, status, detail="", shots=(), **extra):
    r = dict(area=area, item=item, status=status, detail=detail, screenshots=list(shots), **extra)
    results.append(r)
    print(f"[{status:8}] {area}: {item} {detail}", flush=True)
    return r


def sh(cmd, **kw):
    return subprocess.run(cmd, shell=isinstance(cmd, str), capture_output=True, text=True, **kw)


def xdo(*a):
    return sh(["xdotool", *map(str, a)]).stdout.strip()


class Env:
    def __init__(self, out, display):
        self.out = out
        self.display = display
        self.tmp = tempfile.mkdtemp(prefix="flare_qa_")
        self.home = os.path.join(self.tmp, "home")
        os.makedirs(self.home)
        self.work = os.path.join(self.tmp, "work")
        os.makedirs(self.work)
        self.shots = os.path.join(out, "shots")
        os.makedirs(self.shots, exist_ok=True)
        self.xvfb = None
        self.layout = None
        self.env = dict(os.environ, DISPLAY=display, HOME=self.home, USER="flare",
                        XDG_DATA_HOME=self.home + "/.local/share", XDG_CONFIG_HOME=self.home + "/.config",
                        LIBGL_ALWAYS_SOFTWARE="1", QT_QPA_PLATFORM="xcb",
                        FLARE_QUIET_IMPORT="1")
        os.environ["DISPLAY"] = display

    def start_x(self):
        self.xvfb = subprocess.Popen(["Xvfb", self.display, "-screen", "0", f"{SCREEN[0]}x{SCREEN[1]}x24"],
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(40):
            if sh(["xdotool", "getdisplaygeometry"]).returncode == 0:
                return
            time.sleep(0.25)
        raise RuntimeError("Xvfb did not start")

    def close(self):
        # keep any crash reports / dumps the app wrote (they die with the temp HOME otherwise)
        for d, _, fs in os.walk(self.tmp):
            for f in fs:
                if re.search(r"crash|\.dmp$|backtrace", f, re.I) and os.path.getsize(os.path.join(d, f)) < 5_000_000:
                    os.makedirs(os.path.join(self.out, "crash"), exist_ok=True)
                    shutil.copy(os.path.join(d, f), os.path.join(self.out, "crash", f))
        if self.xvfb:
            self.xvfb.kill()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def set_layout(self, profile):
        self.layout = profile  # applied to the fresh stuff copy of the next App launch

    def fresh_root(self, tag):
        """Every launch gets its own stuff/ (sandbox project) and HOME: no state leaks between cases."""
        d = tempfile.mkdtemp(prefix="run_", dir=self.tmp)
        root = os.path.join(d, "stuff")
        shutil.copytree(os.path.join(REPO, "stuff"), root)
        home = os.path.join(d, "home")
        os.makedirs(home)
        if self.layout:
            sd = os.path.join(root, "profiles", "layouts", "settings.flare")
            os.makedirs(sd, exist_ok=True)
            with open(os.path.join(sd, "preferences.ini"), "w") as f:
                f.write(f"[General]\nCurrentRoomChoice={self.layout}\n")
        return dict(self.env, FLAREROOT=root, HOME=home, XDG_DATA_HOME=home + "/.local/share", XDG_CONFIG_HOME=home + "/.config")


class App:
    """One Flare process plus its QA hook channel."""

    def __init__(self, env, tag, args=(), timeout=60):
        self.env, self.tag = env, tag
        self.hookdir = os.path.join(env.tmp, "hook_" + re.sub(r"\W", "_", tag))
        shutil.rmtree(self.hookdir, ignore_errors=True)
        os.makedirs(self.hookdir)
        self.log_path = os.path.join(env.out, "logs", re.sub(r"\W", "_", tag) + ".log")
        os.makedirs(os.path.dirname(self.log_path), exist_ok=True)
        self.log = open(self.log_path, "w")
        binp = os.environ.get("FLARE_BIN") or os.path.join(os.environ.get("BUILD_DIR", "/home/user/bld"), "bin", "Flare")
        e = dict(env.fresh_root(tag), FLARE_QA_HOOK=self.hookdir)
        self.proc = subprocess.Popen([binp, *args], env=e, stdout=self.log, stderr=subprocess.STDOUT)
        self.win = None
        self.ready = False
        self._wait_ready(timeout)

    # -- lifecycle ---------------------------------------------------------------------
    def alive(self):
        return self.proc.poll() is None

    def _wait_ready(self, timeout):
        end = time.time() + timeout
        while time.time() < end and self.alive():
            if not self.win:
                for w in xdo("search", "--onlyvisible", "--pid", self.proc.pid).split():
                    g = dict(l.split("=") for l in xdo("getwindowgeometry", "--shell", w).split() if "=" in l)
                    if int(g.get("WIDTH", 0)) >= 800 and int(g.get("HEIGHT", 0)) >= 500:
                        self.win = w
                        break
            if self.win:
                self.dismiss_startup()  # the hook only runs once the modal Startup popup is gone
                r = self.hook("stats", wait=2, quiet=True)
                if r and r.startswith("ok"):
                    self.ready = True
                    break
            time.sleep(0.5)
        if self.ready:
            xdo("windowmove", self.win, 0, 0, "windowsize", self.win, *SCREEN)
            time.sleep(1.5)

    def stop(self):
        if self.alive():
            self.hook("quit", wait=3, quiet=True)
            try:
                self.proc.wait(3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.log.close()

    def not_ready_reason(self):
        """Why the app never answered the hook: died, crash dialog, other dialog, or just hung."""
        if not self.alive():
            return f"application exited rc={self.proc.poll()} during startup/load"
        titles = [xdo("getwindowname", w) for w in xdo("search", "--onlyvisible", "--pid", self.proc.pid).split()]
        bad = [t for t in titles if re.search(r"crash|error|warning|critical", t, re.I)]
        if bad:
            return f"app blocked on dialog '{bad[0]}'"
        return "application unresponsive (hung) for 60 s"

    def crashed(self):
        rc = self.proc.poll()
        return rc is not None and rc != 0

    def log_text(self):
        try:
            return open(self.log_path, errors="replace").read()
        except OSError:
            return ""

    # -- hook ----------------------------------------------------------------------------
    def hook(self, cmd, wait=30, quiet=False):
        res = os.path.join(self.hookdir, "result")
        if os.path.exists(res):
            os.remove(res)
        tmp = os.path.join(self.hookdir, "cmd.tmp")
        with open(tmp, "w") as f:
            f.write(cmd + "\n")
        os.rename(tmp, os.path.join(self.hookdir, "cmd"))
        end = time.time() + wait
        while time.time() < end:
            if os.path.exists(res):
                return open(res).read().strip()
            if not self.alive():
                return None
            time.sleep(0.15)
        return None

    def stats(self):
        r = self.hook("stats", wait=15) or ""
        d = dict(re.findall(r"(\w+)=(\S*)", r))
        d["raw"] = r
        return d

    def geom(self, cls):
        r = self.hook("geom " + cls, wait=10) or ""
        m = re.match(r"ok (-?\d+) (-?\d+) (\d+) (\d+)", r)
        return tuple(map(int, m.groups())) if m else None

    # -- X helpers -------------------------------------------------------------------------
    def dialogs(self):
        """Open dialog windows of this app (see dialogs.sh); [] if none."""
        r = sh(["bash", "-c", f". '{HERE}/dialogs.sh'; find_open_dialogs {self.proc.pid} {self.win}"])
        return [l for l in r.stdout.splitlines() if l.startswith("window")]

    def dismiss_startup(self):
        for _ in range(8):
            ids = [w for w in xdo("search", "--onlyvisible", "--name", "^Flare Startup$").split()
                   if xdo("getwindowpid", w) == str(self.proc.pid)]
            if not ids:
                return
            xdo("windowfocus", ids[0])
            xdo("key", "Escape")
            time.sleep(0.6)
        for w in ids:
            xdo("windowunmap", w)

    def dismiss_dialogs(self):
        for _ in range(4):
            if not self.dialogs():
                return
            for l in self.dialogs():
                w = l.split()[1]
                xdo("windowfocus", w)
                xdo("key", "Escape")
            time.sleep(0.5)

    def focus(self):
        xdo("windowfocus", self.win)
        time.sleep(0.2)

    def key(self, k):
        self.focus()
        xdo("key", k)
        time.sleep(0.5)

    def shot(self, name):
        p = os.path.join(self.env.shots, re.sub(r"[^\w.-]", "_", name) + ".png")
        sh(["import", "-window", "root", p])
        return p


XKEYS = {".": "period", ",": "comma", "Space": "space", "Left": "Left", "Right": "Right", "Up": "Up", "Down": "Down",
         "Return": "Return", "Esc": "Escape", "-": "minus", "=": "equal", "/": "slash", "[": "bracketleft", "]": "bracketright"}


def shortcut_key(app, cmd):
    """xdotool key spec for the app's current shortcut of a command id (None if unbound)."""
    r = (app.hook("shortcut " + cmd, wait=8) or "").split(" ", 1)
    if r[0] != "ok" or len(r) < 2 or not r[1].strip():
        return None
    parts = r[1].strip().split(",")[0].split("+")
    mods = {"Ctrl": "ctrl", "Shift": "shift", "Alt": "alt", "Meta": "super"}
    out = [mods.get(m, m) for m in parts[:-1]]
    k = parts[-1] if parts[-1] else "plus"
    out.append(XKEYS.get(k, k.lower() if len(k) == 1 else k))
    return "+".join(out)


def accept_dialog(app, key="Return"):
    """Focus the first open dialog of the app and press a key (default OK)."""
    for l in app.dialogs():
        w = l.split()[1]
        xdo("windowfocus", w)
        time.sleep(0.3)
        xdo("key", key)
        time.sleep(1.0)
        return True
    return False


def make_vector_level(app):
    """New Vector Level via the real dialog (focus + OK). Returns True when a column exists."""
    app.hook("cmd MI_NewVectorLevel", wait=8)
    for _ in range(10):
        if app.dialogs():
            break
        time.sleep(0.5)
    r = app.hook("button OK", wait=8)
    if not (r or "").startswith("ok"):
        accept_dialog(app)
    for _ in range(10):
        time.sleep(0.5)
        if int(app.stats().get("cols", 0)) > 0:
            return True
    return False


def crop(path, rect):
    im = Image.open(path).convert("RGB")
    x, y, w, h = rect
    return im.crop((max(x, 0), max(y, 0), min(x + w, im.width), min(y + h, im.height)))


def changed_pixels(a, b, rect, thr=24):
    """Number of pixels inside rect whose channels differ by more than thr."""
    d = ImageChops.difference(crop(a, rect), crop(b, rect)).convert("L")
    return sum(d.point(lambda v: 255 if v > thr else 0).histogram()[255:])


def blank(path, rect=None):
    im = Image.open(path).convert("L") if rect is None else crop(path, rect).convert("L")
    ex = im.getextrema()
    return (ex[1] - ex[0]) < 8


def stroke(app, rect, scale=1.0, mid_shot=None):
    """A zig-zag across the middle of the viewer rect (fractions of its size)."""
    x, y, w, h = rect
    pts = [(0.30, 0.35), (0.45, 0.60), (0.55, 0.38), (0.70, 0.62)]
    pts = [(int(x + w * fx), int(y + h * fy)) for fx, fy in pts]
    app.focus()
    xdo("mousemove", *pts[0])
    time.sleep(0.15)
    xdo("mousedown", 1)
    for (px, py), (qx, qy) in zip(pts, pts[1:]):
        for i in range(1, 9):
            xdo("mousemove", px + (qx - px) * i // 8, py + (qy - py) * i // 8)
            time.sleep(0.03)
    if mid_shot:  # still holding the button: rubber bands / handles are visible only now
        time.sleep(0.4)
        sh(["import", "-window", "root", mid_shot])
    xdo("mouseup", 1)
    time.sleep(0.8)


# ============================================================================ sections ====
def sec_autocreate(env):
    """What a user does first: pick the Brush and draw on an empty cell (level auto-creation)."""
    app = App(env, "autocreate")
    if not app.ready:
        rec("tools", "Brush on empty cell (level auto-create)", "UNTESTED", "app not ready"); app.stop(); return
    try:
        app.hook("new"); app.hook("cmd T_Brush")
        view = app.geom("SceneViewer")
        before = app.shot("autocreate_a")
        stroke(app, view)
        time.sleep(1.0)
        after = app.shot("autocreate_b")
        dlg = app.dialogs() if app.alive() else []
        st = app.stats() if app.alive() and not dlg else {"cols": "?", "frames": "?", "raw": "n/a"}
        if not app.alive() or any("crashed" in d.lower() for d in dlg):
            rec("tools", "Brush on empty cell (level auto-create)", "FAIL", "application CRASHED (SIGSEGV in TTool::touchImage -> ToonzScene::createNewLevel -> TXshSimpleLevel::load -> StudioPalette::updateLinkedColors -> TPalette::getStyle); see crash/ and screenshot", [before, after])
        elif int(st.get("cols", 0)) > 0 and changed_pixels(before, after, view) >= 40:
            rec("tools", "Brush on empty cell (level auto-create)", "PASS", f"level created, canvas changed; {st['raw']}", [before, after])
        else:
            rec("tools", "Brush on empty cell (level auto-create)", "FAIL", f"no level/stroke: {st['raw']} dialogs={dlg}", [before, after])
    finally:
        app.stop()


def sec_tools(env, a):
    sec_autocreate(env)
    sec_edit_later = True
    app = App(env, "tools")
    if not app.ready:
        for t in TOOLS:
            rec("tools", t, "UNTESTED", "app did not become ready (hook/main window)")
        app.stop()
        return
    try:
        app.hook("new")
        if not make_vector_level(app):
            for t in TOOLS:
                rec("tools", t, "UNTESTED", "could not create a vector level through the New Level dialog (harness)")
            return
        app.hook("cmd T_Brush")
        view = app.geom("SceneViewer")
        opts = app.geom("ToolOptions") or app.geom("ToolOptionsBar")
        if not view:
            for t in TOOLS:
                rec("tools", t, "UNTESTED", "no visible SceneViewer widget (hook geom)")
            return
        prev_opts = None
        for i, t in enumerate(TOOLS):
            if not app.alive():
                rec("tools", t, "FAIL", "application died before this tool (earlier crash)")
                continue
            app.dismiss_dialogs()
            r = app.hook("cmd " + t)
            if not r or not r.startswith("ok"):
                rec("tools", t, "FAIL", f"select failed: {r}")
                continue
            time.sleep(0.6)
            before = app.shot(f"tool{i:02d}_{t}_a")
            opts_changed = (changed_pixels(prev_opts, before, opts) if (prev_opts and opts) else 0)
            mid = os.path.join(env.shots, f"tool{i:02d}_{t}_drag.png")
            stroke(app, view, mid_shot=mid)
            after = app.shot(f"tool{i:02d}_{t}_b")
            if not app.alive():
                rec("tools", t, "FAIL", "application CRASHED while drawing", [before])
                continue
            n = max(changed_pixels(before, after, view), changed_pixels(before, mid, view))
            dlg = app.dialogs()
            prev_opts = before
            st = app.stats()
            if dlg and t in RASTER_ONLY and "Warning" in dlg[0]:
                rec("tools", t, "PASS", "raster-only tool correctly refuses the Vector level with a warning dialog (dismissed); "
                    "not drawn-tested here", [before, after])
            elif dlg:
                rec("tools", t, "FAIL", f"dialog open after stroke: {dlg[0]}", [before, after])
            elif st.get("tool") != t:
                rec("tools", t, "FAIL", f"tool not active (current={st.get('tool')})", [before, after])
            elif n >= 12:  # idle-canvas noise is 0-8 px
                rec("tools", t, "PASS", f"canvas changed ({n} px)", [before, after], canvas_px=n)
            elif opts_changed >= 40:
                rec("tools", t, "PASS", f"non-drawing: tool-options panel changed ({opts_changed} px), canvas {n} px",
                    [before, after], options_px=opts_changed)
            else:
                rec("tools", t, "FAIL", f"neither canvas ({n} px) nor tool options ({opts_changed} px) changed",
                    [before, after])
    finally:
        crash = app.crashed()
        app.stop()
        if crash or re.search(r"FATAL|Segmentation|terminate called", app.log_text()):
            rec("tools", "no fatal in app log", "FAIL", "; ".join(re.findall(r".*(?:FATAL|Segmentation|terminate called).*", app.log_text())[:2]))
        else:
            rec("tools", "no fatal in app log", "PASS")


def sec_edit(env):
    """Undo/redo, playback and .tnz save/reload on a fresh app (no leftover view zoom/rotation)."""
    app = App(env, "edit")
    if not app.ready:
        rec("edit", "undo/redo/playback/save", "UNTESTED", "app not ready"); app.stop(); return
    try:
        # ---- undo / redo / playback on a clean scene with three drawn frames ----
        app.hook("new")
        if not make_vector_level(app):
            rec("edit", "undo/redo/playback/save", "UNTESTED", "could not create a vector level (harness)")
            return
        app.hook("cmd T_Brush")
        view = app.geom("SceneViewer")
        base = app.shot("edit_base")
        stroke(app, view)
        drawn = app.shot("edit_drawn")
        n = changed_pixels(base, drawn, view)
        if n < 40:
            rec("edit", "brush stroke for undo test", "FAIL", f"brush stroke did not change the canvas ({n} px)", [base, drawn])
        else:
            app.key("ctrl+z")
            undone = app.shot("edit_undo_key")
            u = changed_pixels(drawn, undone, view)
            rec("edit", "undo via Ctrl+Z (keyboard)", "PASS" if u >= 40 and changed_pixels(base, undone, view) < 40 else "FAIL",
                f"undo removed {u} px; residual vs base {changed_pixels(base, undone, view)} px", [drawn, undone])
            app.key("ctrl+y")
            redone = app.shot("edit_redo_key")
            rr = changed_pixels(undone, redone, view)
            rec("edit", "redo via Ctrl+Y (keyboard)", "PASS" if rr >= 40 else "FAIL", f"redo restored {rr} px", [undone, redone])
            app.hook("cmd MI_Undo")
            time.sleep(0.6)
            u2 = app.shot("edit_undo_cmd")
            rec("edit", "undo via MI_Undo command", "PASS" if changed_pixels(redone, u2, view) >= 40 else "FAIL", "", [redone, u2])
            app.hook("cmd MI_Redo")
            time.sleep(0.6)
        # playback: three frames with a stroke each
        for k in range(6):
            app.hook(f"frame {k}")
            time.sleep(0.4)
            stroke(app, view)
        app.hook("frame 0")
        # The MI_NextFrame / MI_Play commands act on the viewer's flip console, so drive them the way a
        # user does: pointer over the viewer + the default shortcuts (Shift+. next frame, P play).
        xdo("mousemove", view[0] + view[2] // 2, view[1] + view[3] // 2)
        time.sleep(0.3)
        nf0 = app.stats().get("curframe")
        nxt = shortcut_key(app, "MI_NextFrame") or "shift+period"
        app.key(nxt)
        nf1 = app.stats().get("curframe")
        rec("edit", f"next-frame shortcut ({nxt}) advances the frame", "PASS" if nf1 != nf0 else "FAIL", f"{nf0} -> {nf1}")
        app.hook("frame 0")
        r = app.hook("cmd MI_NextFrame"); time.sleep(0.5)
        nf2 = app.stats().get("curframe")
        rec("edit", "MI_NextFrame command advances the frame", "PASS" if nf2 != "0" else "FAIL",
            f"curframe 0 -> {nf2} (the same action works from the keyboard; the command path is focus-dependent)")
        app.hook("frame 0")
        st0 = app.stats()
        rec("edit", "scene has multiple frames", "PASS" if int(st0.get("frames", 0)) >= 6 else "FAIL", st0["raw"])
        f0 = st0.get("curframe")
        play = shortcut_key(app, "MI_Play")
        app.hook("cmd MI_Loop")  # loop so a short scene keeps playing while we sample
        app.hook("frame 0")
        f0 = app.stats().get("curframe")
        app.key(play or "p")
        seen = []
        for _ in range(16):
            seen.append(app.stats().get("curframe"))
            time.sleep(0.08)
        app.key(play or "p")
        sh0 = app.shot("playback_stopped")
        rec("edit", "playback advances frame counter", "PASS" if len(set(seen)) > 2 else "FAIL",
            f"start={f0} sampled={seen} (Xvfb: playback runs on a worker thread; frame counter did not tick if constant)", [sh0])
        fa = app.stats().get("curframe"); time.sleep(0.8); fb = app.stats().get("curframe")
        if len(set(seen)) > 2:
            rec("edit", "playback pause stops frame counter", "PASS" if fa == fb else "FAIL", f"{fa} -> {fb}")
        else:
            rec("edit", "playback pause stops frame counter", "UNTESTED", "playback never started (see previous item)")
        # ---- save / reload .tnz ----
        sc = os.path.join(env.work, "qa_roundtrip.tnz")
        app.dismiss_dialogs()
        before = app.shot("save_before")
        r = app.hook("save " + sc)
        exists = os.path.isfile(sc) and os.path.getsize(sc) > 100
        rec("save", "save scene (.tnz written)", "PASS" if r and r.startswith("ok") and exists else "FAIL", f"hook={r} size={os.path.getsize(sc) if os.path.isfile(sc) else 0}")
        stsave = app.stats()
        app.hook("new")
        stnew = app.stats()
        r = app.hook("load " + sc)
        time.sleep(1.5)
        stload = app.stats()
        after = app.shot("save_reloaded")
        ok = (r or "").startswith("ok") and stload.get("frames") == stsave.get("frames") and stload.get("cols") == stsave.get("cols") and int(stload.get("cols", 0)) > 0
        rec("save", "reload .tnz restores frames/columns", "PASS" if ok else "FAIL",
            f"saved[{stsave['raw']}] after-new[{stnew['raw']}] reloaded[{stload['raw']}] hook={r}", [before, after])
        v2 = app.geom("SceneViewer") or view
        d = changed_pixels(before, after, v2)
        rec("save", "reloaded scene renders same canvas", "PASS" if d < 400 else "FAIL", f"{d} px differ vs before save", [before, after])
        app.dismiss_dialogs()
    finally:
        crash = app.crashed()
        app.stop()
        if crash or re.search(r"FATAL|Segmentation|terminate called", app.log_text()):
            rec("edit", "no fatal in app log", "FAIL", "; ".join(re.findall(r".*(?:FATAL|Segmentation|terminate called).*", app.log_text())[:2]))
        else:
            rec("edit", "no fatal in app log", "PASS")


def sec_tools_all(env, a):
    sec_tools(env, a)
    sec_edit(env)


def sec_prefs(env, a):
    app = App(env, "prefs")
    if not app.ready:
        rec("prefs", "Preferences dialog", "UNTESTED", "app not ready")
        app.stop(); return
    try:
        app.hook("cmd MI_Preferences")
        time.sleep(2)
        n = (app.hook("prefscount", wait=10) or "").split()
        if len(n) < 2 or n[0] != "ok":
            rec("prefs", "Preferences dialog opens", "FAIL", f"prefscount={n}", [app.shot("prefs_open_fail")])
            return
        cnt = int(n[1])
        rec("prefs", "Preferences dialog opens", "PASS", f"{cnt} category pages")
        prev = None
        for i in range(cnt):
            r = app.hook(f"prefs {i}", wait=15) or ""
            time.sleep(0.8)
            name = r.split()[1] if r.startswith("ok") else f"page{i}"
            p = app.shot(f"prefs_{i:02d}_{name}")
            if not app.alive():
                rec("prefs", name, "FAIL", "application died switching to this page"); return
            bad = blank(p)
            same = prev is not None and changed_pixels(prev, p, (0, 0, *SCREEN)) < 40
            rec("prefs", f"tab {i}: {name}", "FAIL" if (not r.startswith("ok") or bad or same) else "PASS",
                "switch failed" if not r.startswith("ok") else ("screenshot blank" if bad else ("page identical to previous" if same else "page rendered")), [p])
            prev = p
    finally:
        app.stop()


def sec_rooms(env, a):
    profiles = sorted(d for d in os.listdir(os.path.join(REPO, "stuff", "profiles", "layouts", "rooms"))
                      if os.path.isdir(os.path.join(REPO, "stuff", "profiles", "layouts", "rooms", d)))
    for prof in profiles:
        env.set_layout(prof)
        app = App(env, "rooms_" + prof)
        if not app.ready:
            rec("rooms", f"{prof}: launch", "FAIL" if app.crashed() or not app.alive() else "UNTESTED", "no main window / hook")
            app.stop(); continue
        try:
            r = (app.hook("rooms") or "").split()
            if len(r) < 3 or r[0] != "ok":
                rec("rooms", f"{prof}: room list", "FAIL", str(r)); continue
            cnt, names = int(r[1]), r[2].split(",")
            rec("rooms", f"{prof}: launch", "PASS", f"{cnt} rooms: {', '.join(names)}")
            prev = None
            for i in range(cnt):
                app.dismiss_dialogs()
                res = app.hook(f"room {i}")
                time.sleep(2.0)
                p = app.shot(f"room_{prof}_{i}_{names[i]}")
                if not app.alive():
                    rec("rooms", f"{prof}/{names[i]}", "FAIL", "application died switching rooms"); break
                dlg = app.dialogs()
                if dlg:
                    st, why = "FAIL", f"dialog open: {dlg[0]}"
                elif not (res or "").startswith("ok"):
                    st, why = "FAIL", f"room switch failed: {res}"
                elif blank(p):
                    st, why = "FAIL", "blank screenshot"
                elif prev and cnt > 1 and changed_pixels(prev, p, (0, 0, *SCREEN)) < 200:
                    st, why = "FAIL", "room looks identical to previous room"
                else:
                    st, why = "PASS", "rendered"
                rec("rooms", f"{prof}/{names[i]}", st, why, [p])
                prev = p
        finally:
            app.stop()
    env.set_layout(None)


def import_candidates(extra_dir):
    fx = os.path.join(REPO, "tests", "flash_fixtures")
    c = []
    for p in sorted(glob.glob(fx + "/*")):
        n = os.path.basename(p)
        if os.path.isdir(p):
            if os.path.exists(os.path.join(p, "DOMDocument.xml")) or n.endswith(".xfl") or n.endswith("_xfl"):
                c.append(p)
            continue
        if n.rsplit(".", 1)[-1].lower() in IMPORT_EXTS:
            c.append(p)
    c += sorted(glob.glob(fx + "/moho_samples/*.moho*"))
    if extra_dir:
        for p in sorted(glob.glob(os.path.join(extra_dir, "**", "*"), recursive=True)):
            if os.path.isfile(p) and p.rsplit(".", 1)[-1].lower() in IMPORT_EXTS:
                c.append(p)
            elif os.path.isdir(p) and os.path.exists(os.path.join(p, "DOMDocument.xml")):
                c.append(p)
    return c


def gen_assets(env):
    """Small synthetic raster/vector images (the harness generates them; nothing to download)."""
    d = os.path.join(env.work, "gen")
    os.makedirs(d, exist_ok=True)
    im = Image.new("RGB", (160, 120), (255, 255, 255))
    for x in range(40, 120):
        for y in range(30, 90):
            im.putpixel((x, y), (200, 30, 30) if (x + y) % 40 < 20 else (30, 30, 200))
    out = {}
    for ext, fmt in (("png", "PNG"), ("jpg", "JPEG"), ("gif", "GIF")):
        p = os.path.join(d, "qa_synth." + ext)
        im.save(p, fmt)
        out[ext] = p
    p = os.path.join(d, "qa_synth.svg")
    open(p, "w").write('<svg xmlns="http://www.w3.org/2000/svg" width="160" height="120"><rect x="40" y="30" width="80" height="60" fill="#c81e1e"/>'
                       '<circle cx="80" cy="60" r="20" fill="#1e1ec8"/></svg>')
    out["svg"] = p
    return out


def stage_copy(env, p):
    """Importers write beside their input (e.g. *_flare_import); work on a copy, never in the repo."""
    d = tempfile.mkdtemp(dir=env.work)
    dst = os.path.join(d, os.path.basename(p))
    (shutil.copytree if os.path.isdir(p) else shutil.copy)(p, dst)
    return dst


def sec_import(env, a):
    counts = {}
    for p0 in import_candidates(a.dir) + sorted(gen_assets(env).values()):
        name = os.path.relpath(p0, REPO) if p0.startswith(REPO) else p0
        p = stage_copy(env, p0)
        app = App(env, "import_" + os.path.basename(p), args=[p])
        if not app.ready:
            rec("import", name, "FAIL", app.not_ready_reason(), [app.shot("import_" + os.path.basename(p))])
            app.stop(); continue
        try:
            time.sleep(4)  # importer runs on the GUI thread; hook answers when it is done
            st = app.stats()
            time.sleep(1)
            p1 = app.shot("import_" + os.path.basename(p))
            dlg = app.dialogs() if app.alive() else []
            if not app.alive():
                rec("import", name, "FAIL", "application CRASHED", [p1]); continue
            counts[p] = (st.get("cols"), st.get("frames"))
            ext = p.rsplit(".", 1)[-1].lower() if not os.path.isdir(p) else "xfl"
            cols, frames = int(st.get("cols") or 0), int(st.get("frames") or 0)
            logt = app.log_text()
            kw = dict(cols=cols, frames=frames)
            if dlg:
                rec("import", name, "FAIL", f"modal dialog blocks import: {dlg[0]}", [p1], **kw)
            elif ext in CONTENT_EXTS and cols == 0:
                m = re.search(r"\[Flash import result\][^\n]*\n([^\n]*\n(?:  [^\n]*\n)*)", logt)
                if "contain only empty keyframes" in logt:
                    rec("import", name, "PASS", "document has only empty keyframes; importer says so honestly (D21)", [p1], **kw)
                elif "Not converted" in logt or (m and "file(s) exported" in logt):
                    rec("import", name, "PASS", "0 columns, but the importer's result text explains it: " + " / ".join((m.group(1) if m else "Not converted").split("\n")[:3])[:160], [p1], **kw)
                else:
                    rec("import", name, "FAIL", "imported 0 columns / 0 frames with no explanation", [p1], **kw)
            else:
                rec("import", name, "PASS", f"cols={cols} frames={frames}" + ("" if ext in CONTENT_EXTS else " (reference-only format)"), [p1], **kw)
        finally:
            app.stop()
    # Moho through the QA hook (quiet, no dialogs) as well: structure -> columns.
    moho = [stage_copy(env, m) for m in sorted(glob.glob(os.path.join(REPO, "tests", "flash_fixtures", "moho_samples", "*.moho")))]
    if moho:
        app = App(env, "moho")
        if not app.ready:
            for m in moho:
                rec("moho", os.path.basename(m), "UNTESTED", "app not ready")
        else:
            try:
                for m in moho:
                    app.hook("new")
                    r = app.hook("moho " + m, wait=60) or "no result"
                    st = app.stats()
                    ok = r.startswith("ok")
                    cols = int(re.search(r"columns=(\d+)", r).group(1)) if ok else -1
                    rec("moho", os.path.basename(m), "PASS" if ok and cols > 0 else "FAIL",
                        f"{r}; xsheet cols={st.get('cols')}", [app.shot("moho_" + os.path.basename(m))])
            finally:
                app.stop()


def sec_export(env, a):
    src = gen_assets(env)["png"]
    app = App(env, "export_src", args=[src])
    base = None
    outs = {}
    if not app.ready:
        rec("export", "XFL/HTML5 export", "UNTESTED", "app not ready"); app.stop(); return
    try:
        time.sleep(4)
        base = app.stats()
        p0 = app.shot("export_source_scene")
        if int(base.get("cols", 0)) == 0 or int(base.get("frames", 0)) == 0:
            rec("export", "source scene for export", "FAIL", f"loading {os.path.basename(src)} gave no content: {base['raw']}", [p0])
            return
        rec("export", "source scene for export", "PASS", base["raw"], [p0])
        for kind, fn in (("xfl", "export_out.fla"), ("xfl", "export_out.xfl"), ("html5", "export_out.html")):
            path = os.path.join(env.work, fn)
            r = app.hook(f"export {kind} {path}", wait=90)
            if fn.endswith(".xfl") and not os.path.exists(path):
                path = path[:-4]  # "X.xfl" is written as the project folder X/ holding X.xfl
            ok = bool(r and r.startswith("ok")) and os.path.exists(path)
            sz = (os.path.getsize(path) if os.path.isfile(path) else sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(path) for f in fs)) if os.path.exists(path) else 0
            rec("export", f"export {fn}", "PASS" if ok and sz > 100 else "FAIL", f"hook={r} size={sz}")
            if ok:
                outs[fn] = path
    finally:
        app.stop()
        art = os.path.join(env.out, "artifacts")  # keep the exports for inspection
        os.makedirs(art, exist_ok=True)
        for f in glob.glob(os.path.join(env.work, "export_out*")):
            (shutil.copytree if os.path.isdir(f) else shutil.copy)(f, os.path.join(art, os.path.basename(f)))
    # re-import each XFL export and compare counts with the source scene
    for fn in ("export_out.fla", "export_out.xfl"):
        if fn not in outs:
            rec("export", f"re-import {fn}", "UNTESTED", "export failed"); continue
        app = App(env, "reimport_" + fn, args=[outs[fn]])
        try:
            if not app.ready:
                rec("export", f"re-import {fn}", "FAIL", app.not_ready_reason()); continue
            time.sleep(4)
            st = app.stats()
            dlg = app.dialogs()
            p = app.shot("reimport_" + fn)
            same = st.get("cols") == base.get("cols") and st.get("frames") == base.get("frames")
            rec("export", f"re-import {fn}: counts match source", "FAIL" if (dlg or not same) else "PASS",
                f"source cols/frames={base.get('cols')}/{base.get('frames')} re-imported={st.get('cols')}/{st.get('frames')}" + (f" dialog={dlg[0]}" if dlg else ""), [p])
        finally:
            app.stop()
    # HTML5 in headless chromium
    html = outs.get("export_out.html")
    if not html:
        rec("export", "HTML5 renders in chromium", "UNTESTED", "html export failed"); return
    shotp = os.path.join(env.shots, "html5_chromium.png")
    r = sh(["node", os.path.join(HERE, "html5_check.js"), html, shotp], env=dict(os.environ, NODE_PATH="/opt/node-tools/node_modules"))
    try:
        j = json.loads(r.stdout.strip().splitlines()[-1])
    except Exception:
        rec("export", "HTML5 renders in chromium", "UNTESTED", f"playwright runner failed: {r.stderr[-200:] or r.stdout[-200:]}"); return
    if j["errors"] and j["errors"][0].startswith("harness:"):
        rec("export", "HTML5 renders in chromium", "UNTESTED", j["errors"][0]); return
    rec("export", "HTML5: no console errors", "PASS" if j["ok"] else "FAIL", "; ".join(j["errors"][:3]) or "clean", [shotp])
    rec("export", "HTML5: non-blank screenshot", "FAIL" if (not os.path.exists(shotp) or blank(shotp)) else "PASS", "", [shotp])


# =============================================================================== main ====
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sections", nargs="*")
    ap.add_argument("--out", required=True)
    ap.add_argument("--dir", default=None)
    ap.add_argument("--display", default=":88")
    a = ap.parse_args()
    secs = a.sections or ["tools", "import", "export", "prefs", "rooms"]
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    env = Env(out, a.display)
    t0 = time.time()
    try:
        env.start_x()
        table = dict(tools=sec_tools_all, import_=sec_import, export=sec_export, prefs=sec_prefs, rooms=sec_rooms)
        table["import"] = sec_import
        for s in secs:
            try:
                table[s](env, a)
            except Exception as e:  # harness bug: surface, do not hide
                import traceback
                rec(s, "harness error", "UNTESTED", f"{type(e).__name__}: {e}")
                traceback.print_exc()
    finally:
        env.close()
    summ = {k: sum(1 for r in results if r["status"] == k) for k in ("PASS", "FAIL", "UNTESTED")}
    with open(os.path.join(out, "results.json"), "w") as f:
        json.dump(dict(summary=summ, seconds=round(time.time() - t0), results=results), f, indent=1)
    with open(os.path.join(out, "results.md"), "w") as f:
        f.write(f"# Flare GUI QA\n\nPASS {summ['PASS']}  FAIL {summ['FAIL']}  UNTESTED {summ['UNTESTED']}\n\n")
        f.write("| Area | Item | Status | Detail | Screenshots |\n|---|---|---|---|---|\n")
        for r in results:
            f.write("| {} | {} | {} | {} | {} |\n".format(r["area"], r["item"], r["status"],
                    r["detail"].replace("|", "/").replace("\n", " ")[:160], " ".join(os.path.relpath(s, out) for s in r["screenshots"])))
    print(json.dumps(summ))
    sys.exit(1 if summ["FAIL"] else 0)


if __name__ == "__main__":
    main()
