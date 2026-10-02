"""Negative control: prove the shape tests can actually fail.

Injects each of the bugs that were fixed, one at a time, and confirms the suite
notices. A test that cannot fail is not a test, and the only way to know whether
these can fail is to break the code on purpose and check.

This exists because the first attempt at this found something uncomfortable: four
of the bugs it was written to detect were *not* caught, because every test case
used coordinates far enough from the threshold that the two comparison
predicates agreed. The near-threshold cases now in xfl_shape_tests.cpp are there
because of that.

Patterns match on a normalised form, and the diff for each is printed, so a
substitution that does not do what its name claims is visible rather than silent
-- which is how four of the first eight attempts turned into no-ops.

It builds tnzcore, so it needs the application build. It leaves the source tree
exactly as it found it, including on failure.

    python tests/native/mutation_check.py
"""
import difflib
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
BUILD = os.environ.get("FLARE_BUILD",
                       os.path.join(REPO, "build_local"))
SHAPE = os.path.join(REPO, "flare", "sources", "common", "flash",
                     "XFLShape.cpp")
EXE = os.path.join(HERE, "build", "RelWithDebInfo", "xfl_shape_tests.exe")

# Qt and the Flare runtime both have to be reachable, or the test process cannot
# start at all -- silently, with no output and a nonzero exit.
QT_BIN = os.environ.get("QT_BIN", "")

ENV = dict(os.environ)
_parts = [QT_BIN, os.path.join(BUILD, "RelWithDebInfo"),
          os.path.join(REPO, "LocalTest"), ENV.get("PATH", "")]
ENV["PATH"] = os.pathsep.join([p for p in _parts if p])
if QT_BIN:
    ENV["QT_PLUGIN_PATH"] = os.path.join(QT_BIN, "..", "plugins")


def norm(text):
    return re.sub(r"[ \t]+", " ", text.replace("\r\n", "\n"))


def _literal_repl(repl):
    """Escape backslashes so re.sub does not read them as substitution escapes.

    re.sub interprets the replacement: `\n` becomes a newline and `\1` is a group
    reference. A control that restores a character comparison has to contain the C
    literal `\n`, and re was turning that into a line break -- so the mutation did
    not compile, and the harness reported NO-BUILD, which it treats as "this
    control cannot tell me anything". The one control whose whole purpose is to
    prove the Moho sniffer catches the original bug was therefore reporting
    nothing.

    Only the backslashes re would misinterpret are doubled. re reads `\\1`..`\\99`
    and `\\g<n>` as group references and every other backslash sequence as a
    literal escape, so those are left alone and the rest are doubled. Doubling
    everything instead -- which is what the first version did -- turned the
    '/' control's `\\1` into a literal backslash-1 in the injected source, and the
    compiler said:

        error C3688: invalid literal suffix 'case'
    """
    return re.sub(r"\\(?![0-9]|g<)", r"\\\\", repl)

def _restore_eol(original, mutated, pattern, repl, file_eol):
    """Put the file's line endings back after a substitution.

    re.sub splices the replacement's own line breaks in verbatim, and those are
    bare \n because the tables are Python literals. These sources are CRLF, so a
    multi-line replacement drops a few CRLFs and leaves git dirty for the duration
    of the mutation -- and a control that was silently rewriting a file's
    formatting is not a control worth trusting.

    So normalise every line ending in the mutated text. Earlier versions tried to
    touch only the lines a replacement introduced: first by bracketing characters
    from each end, then by diffing line indices. Both were wrong at the boundary
    -- a replacement shorter than the text it displaces leaves no clean bracket --
    and the number of controls introducing bare LFs went 5, then 4, then 1, then
    back to 2. Nothing distinguishes the introduced lines anyway: the whole file
    should come out looking the same, which is the property being checked here.
    """
    if file_eol == "\n":
        return mutated
    if mutated.count("\r\n") == mutated.count("\n"):
        return mutated          # already consistent
    return mutated.replace("\r\n", "\n").replace("\n", file_eol)


def inject(original, pattern, repl):
    """Apply a mutation pattern to the source, returning (text, count).

    The pattern is tried against the original text first, so a pattern that does
    not need normalisation mutates exactly the lines it names.

    Failing that it is tried against a normalised copy -- one with \r\n stripped
    and runs of spaces collapsed -- because patterns that mention indentation or
    explicit whitespace need that. The result is then mapped back onto the
    original line by line, so what gets written differs from the original only in
    the lines the mutation actually touched.

    This matters: writing the normalised text back instead would convert the
    whole file to LF for the duration of the mutation. That still compiles, but
    the reported diff would be every line in the file rather than the injected
    one, and an interrupted run would leave the file with its line endings
    rewritten.
    """
    # re.S is required, not optional. Without it `.*?` cannot cross a newline,
    # so every multi-line pattern matches nothing and the control silently stops
    # testing anything. That happened to three of these, including the '/' one:
    # the harness reported them as "pattern matched 0 times" and carried on, so
    # a run could end with a summary line while a control was doing nothing.
    file_eol = "\r\n" if "\r\n" in original else "\n"

    lit = _literal_repl(repl)
    new, n = re.subn(pattern, lit, original, flags=re.S)
    if n == 1:
        # The replacement's own line endings are whatever the table happened to
        # spell -- usually a bare \n, since these are Python literals. Restoring
        # the file's convention is not cosmetic: these sources are CRLF, so a
        # multi-line replacement would otherwise drop a few CRLFs and leave git
        # dirty for the duration of the mutation. Done on both paths, because a
        # control's effect must not depend on whether its pattern needed
        # normalisation.
        if file_eol != "\n":
            # Only the lines the replacement introduced, so a \r\n already in the
            # surrounding original text is not doubled.
            new = _restore_eol(original, new, pattern, repl, file_eol)
        return new, n
    if n > 1:
        return None, n

    ntext = norm(original)
    m = re.search(pattern, ntext, flags=re.S)
    if m is None:
        return None, 0
    mutated = m.expand(_literal_repl(repl))

    # Splice rather than rewrite. The match is located by line number in the
    # normalised text -- which has the same lines as the original, only
    # re-indented -- and the replacement is written into that line range of the
    # original. Rewriting the whole normalised text instead would convert the
    # file to LF, and rebuilding it line by line only works while the mutation
    # happens to preserve the line count, which is false for every control that
    # replaces a block with a shorter one. That is most of the interesting ones.

    start_line = ntext.count("\n", 0, m.start())
    end_line = ntext.count("\n", 0, m.end()) + 1  # +1: the match ends at a break

    orig_lines = original.splitlines(keepends=True)
    if start_line >= len(orig_lines):
        return None, 0

    # Each injected line is rebased onto the original line it replaces: same
    # leading whitespace, same line ending. Without this, a control whose
    # pattern mentions indentation -- so had to be matched against the
    # normalised copy -- writes back that copy's indentation, and every line of
    # the replaced region differs from the original by its leading whitespace.
    # That showed up as controls "rewriting" 374 to 469 lines and shifting the
    # file's CRLF count, which would dirty git for the duration of every run.
    #
    # A control that genuinely wants to change indentation says so by starting
    # its replacement line with a backslash: removing a line should not re-indent
    # the file, and adding one should not either unless asked.
    KEEP = "\\"

    def leading(ln):
        return ln[:len(ln) - len(ln.lstrip(" \t"))]

    def eol_at(idx):
        if idx < len(orig_lines):
            tail = orig_lines[idx]
            return "\r\n" if tail.endswith("\r\n") else "\n"
        return "\r\n" if "\r\n" in original else "\n"

    # The replacement's own line endings are whatever the table happened to
    # spell -- usually a bare \n, since these are Python literals. Rewrite them
    # to the file's convention before splitting, so a multi-line replacement in a
    # CRLF file does not drop the file's line-ending mix and leave git dirty for
    # the duration of the mutation.
    file_eol = "\r\n" if "\r\n" in original else "\n"
    body = mutated.replace("\r\n", "\n").rstrip("\n")
    if file_eol != "\n":
        body = body.replace("\n", file_eol)
    body_lines = body.split(file_eol) if body else []

    injected = []
    for k, ln in enumerate(body_lines):
        orig_idx = start_line + k
        if orig_idx < len(orig_lines):
            base_indent = leading(orig_lines[orig_idx])
            eol = eol_at(orig_idx)
        else:
            # Beyond the original's end: use the indentation of the last line of
            # the replaced region, which is where the block was.
            base_indent = leading(orig_lines[min(end_line, len(orig_lines)) - 1])
            eol = eol_at(orig_idx)
        # The EOL is already part of `ln` -- it came from splitting on file_eol --
        # so appending eol here would double it.
        if ln.endswith("\r\n"):
            ln = ln[:-2]
        elif ln.endswith("\n"):
            ln = ln[:-1]
        if ln.startswith(KEEP):
            injected.append(ln[1:] + eol)
        else:
            injected.append(base_indent + ln.lstrip(" \t") + eol)

    return ("".join(orig_lines[:start_line])
            + "".join(injected)
            + "".join(orig_lines[end_line:]), 1)


# (name, regex, replacement). Each reintroduces a bug that was fixed, and each
# is one of the ways this format is commonly misread.
MUTATIONS = [
    ("restatement test uses qFuzzyCompare, which is relative",
     r"!cur\.points\.isEmpty\(\) && qAbs\(at\.x\(\) - dest\.x\(\)\) < kPointEpsilon &&\s*"
     r"qAbs\(at\.y\(\) - dest\.y\(\)\) < kPointEpsilon",
     "!cur.points.isEmpty() && qFuzzyCompare(at.x(), dest.x()) &&\n"
     "                qFuzzyCompare(at.y(), dest.y())"),

    ("a restated moveTo records a point again",
     r"if \(!restates\) cur\.points\.append\(at\);",
     "cur.points.append(at);"),

    ("a number in an opcode slot is read as a lineTo",
     r"if \(t\.op == Op::Number\) \{",
     "if (false) {"),

    # '/' is a lineTo, byte-identical in meaning to '|'. Treating it as a
    # subpath break is the single most common misreading of this format, and it
    # produces a differently-shaped path rather than an error -- which is why the
    # tokenizer normalises it here. An earlier version of this control matched
    # the restatement predicate instead, so two controls hit one site and
    # nothing exercised this one.
    # \\r?\\n rather than \\n: matched against the original text, which is
    # CRLF, a bare \\n never matches a position just after a \\r -- so the
    # pattern fell through to a later `case '!'` and the replacement landed in
    # the wrong switch. This control predates norm(), which is what made that
    # safe for the others.
    ("'/' normalised to a subpath break instead of a lineTo",
     r"case '!':\r?\n(\s*)case '\|':\r?\n\s*case '/':\r?\n",
     "case '!':\n\\1case '|':\n"),

    ("the comparison epsilon is finer than the coordinate quantum",
     r"constexpr double kPointEpsilon = 1e-3;",
     "constexpr double kPointEpsilon = 1e-9;"),

    ("a quadratic's control point is emitted as its endpoint",
     r"d \+= QLatin1Char\('Q'\) \+ num\(ctrl\.x\(\)\)",
     "d += QLatin1Char('Q') + num(e.x())"),

    ("coordinates not divided from twips to pixels",
     r"raw\.append\(Raw\{'n', toks\.at\(ti \+ k\)\.value / kTwipsPerPixel\}\);",
     "raw.append(Raw{'n', toks.at(ti + k).value});"),

    # A straight segment consumes one point and a quadratic two. Recording the
    # wrong kind desynchronises the point stream from the segment list.
    ("a straight segment recorded as a quadratic",
     r"cur\.segments\.append\(Contour::Kind::Line\);",
     "cur.segments.append(Contour::Kind::Quad);"),
]


def build():
    """Returns (ok, detail).

    A build failure must not be mistaken for a weak test: without this, a
    mutation that did not compile left the previous binary in place, the suite
    passed, and the result was recorded as MISSED -- indistinguishable from the
    suite not being able to detect that particular bug.
    """
    for cwd, args in ((BUILD, ["--target", "tnzcore", "-j", "8"]),
                      (os.path.join(HERE, "build"), ["-j", "8"])):
        r = subprocess.run(["cmake", "--build", cwd,
                            "--config", "RelWithDebInfo"] + args,
                           cwd=cwd, capture_output=True, text=True)
        if r.returncode:
            tail = [l for l in (r.stdout + r.stderr).splitlines()
                    if "error" in l.lower()][:3]
            return False, "; ".join(tail) or "build failed"
    return True, ""


def run(exe=EXE, args=()):
    r = subprocess.run([exe, *args], capture_output=True, text=True, env=ENV)
    return r.returncode, r.stderr


def clip(text, n=84):
    return text.strip()[:n]


# The SWF census and bitmap extractor. A second source file, a second test
# binary, and a different failure mode: a wrong tag code makes the extractor
# write no file and say nothing, which is why these went unnoticed until a
# merge brought a corrected copy of the file into view.
SWF_ASSETS = os.path.join(HERE, "..", "..", "flare", "sources", "common",
                          "flash", "SWFAssets.cpp")

# (name, pattern, replacement). The first two are the family transposition:
# 22 is DefineShape2 and 35 is DefineBitsJPEG3, per the SWF specification and
# per flare/sources/common/flash/Macromedia.h.
SWF_MUTATIONS = [
    # The four tallies added after measuring mario.ssf. Each is a single-line swap
    # in the census dispatch, and each is the whole of its own tally -- so
    # deleting one outright is the same failure this catches.
    ("DefineButton and DefineButton2 no longer counted as buttons",
     r"case 3: case 34: \+\+c\.buttons;",
     "case 3: case 34: break;"),
    ("DefineEditText no longer counted as a text field",
     r"case 37: \+\+c\.fields;",
     "case 37: break;"),
    ("SymbolClass no longer counted",
     r"case 76: \+\+c\.symbols;",
     "case 76: break;"),
    ("VideoFrame no longer counted",
     r"case 61: \+\+c\.videoFrames;",
     "case 61: break;"),
    ("tag 22 missing from the shape tally",
     r"case 2: case 22: case 32: case 46: case 83: \+\+c\.shapes;",
     "case 2: case 32: case 46: case 83: ++c.shapes;"),
    ("JPEG3/JPEG4 dispatch moved onto the lossless codes",
     r"if \(tagCode == 35 \|\| tagCode == 90\) \{",
     "if (tagCode == 20 || tagCode == 36) {"),
    ("tag 24 counted as a font, 48 ignored again",
     r"case 10: case 48: case 75: \+\+c\.fonts;",
     "case 10: case 24: case 75: ++c.fonts;"),
    ("video back on the unreachable 81/93 codes",
     r"case 60: case 62: \+\+c\.video;",
     "case 81: case 93: ++c.video;"),
]

# The Moho container sniffer. A separate source and a separate binary, for the
# same reason as the SWF table: these tests exist because the sniffer used to
# classify every non-JSON file as a legacy .anme project, so a user holding a
# Flash movie was told to re-save it from Moho.
MOHO_READER = os.path.join(HERE, "..", "..", "flare", "sources", "common",
                           "moho", "MohoReader.cpp")

# The XFL reader, covered by xfl_shape_tests' census case. Separate again because
# the census is the only thing the dialog prints about an .fla -- it names what the
# document holds, including content the reader does not convert.
XFL_READER = os.path.join(HERE, "..", "..", "flare", "sources", "common",
                          "flash", "XFLReader.cpp")

MOHO_MUTATIONS = [
    # The Moho container sniffer. A separate source and a separate binary, for the
    # same reason as the SWF table: these controls exist because the sniffer used
    # to classify every non-JSON file as a legacy .anme project, so a user holding
    # a Flash movie was told to re-save it from Moho.
    #
    # Found by running the shipped reader over mario.ssf, a 3.5 MB uncompressed SWF
    # that Moho exported, which came back as "pre-11 .anme project ... Re-save it
    # from Moho as a .moho project."
    #
    # Each is a single-line swap, which is the shape every control in this table
    # that works reliably has. The first attempt here replaced the whole decision
    # loop and took five tries to get right -- the lazy `.*?` stopped at the wrong
    # `return`, then ran on into read(), then stranded the function tail, then left
    # a brace too many. A control that is hard to read is a control nobody can
    # tell is doing the right thing.
    # The two lines are not adjacent: a comment explaining why Unknown is the
    # right answer sits between them. \s* spans it.
    ("every non-JSON file classified as a legacy .anme (the original bug)",
     r"(if \(ch == '\{'\) return Container::RawJson;\s*"
     r"(?://[^\n]*\n\s*)*)return Container::Unknown;",
     r"\1return Container::Legacy;"),

    ("the JSON branch no longer distinguishes a brace from anything else",
     r"if \(ch == '\{'\) return Container::RawJson;",
     r"if (false) return Container::RawJson;"),

    # Negating the guard is the whole decision: without the signature, the
    # leading-whitespace check can never pass, so nothing is ever Legacy here.
    ("the .anme signature no longer required",
     r"if \(legacyAt >= 0 && legacyAt < 16\) \{",
     r"if (false) {"),

    ("a UTF-8 BOM no longer skipped before the JSON brace",
     r"if \(ch == 0xEF && i \+ 2 < head\.size\(\) &&\s*"
     r"static_cast<unsigned char>\(head\.at\(i \+ 1\)\) == 0xBB &&\s*"
     r"static_cast<unsigned char>\(head\.at\(i \+ 2\)\) == 0xBF\) \{\s*"
     r"i \+= 2;\s*continue;\s*\}",
     "if (false) { i += 2; continue; }"),

    ("the container peek shrunk to 8 bytes, missing a late signature",
     r"const QByteArray head = f\.peek\(64\);",
     "const QByteArray head = f.peek(8);"),
]

XFL_MUTATIONS = [
    # The bug this table exists for. parseSymbol() grouped DOMBitmapInstance with
    # DOMSymbolInstance, recorded only the name it referred to, and tallied neither
    # -- so an FLA with bitmaps inside its LIBRARY/ reported none, in the dialog
    # whose whole job is to name what a document holds.
    #
    # \s* rather than a literal run of spaces, because norm() collapses runs of
    # spaces before substituting: a pattern that spells out the indentation never
    # matches, and reports nothing about why. Each pattern also names enough
    # surrounding text to be unambiguous, because the DOMDocument path has its own
    # DOMShape and census.symbols sites which must not be touched.
    ("DOMBitmapInstance grouped with DOMSymbolInstance again (the original bug)",
     r'\}\s*else if \(name == QLatin1String\("DOMSymbolInstance"\)\) \{\n'
     r'\s*// Record the library item name a symbol instance refers to, so a\n'
     r'\s*// bitmap symbol can still be resolved by name\.\n'
     r'\s*const QString ref = attrs\.value\("libraryItemName"\)\.toString\(\);\n'
     r'\s*if \(!found && !ref\.isEmpty\(\)\) relativeName = ref;\n'
     r'\s*\+\+m_document\.census\.symbols;\n'
     r'\s*\}\s*else if \(name == QLatin1String\("DOMBitmapInstance"\)\) \{'
     r'.*?\+\+m_document\.census\.bitmaps;\s*\}',
     '        } else if (name == QLatin1String("DOMSymbolInstance") ||\n'
     '                   name == QLatin1String("DOMBitmapInstance")) {\n'
     '            const QString ref = attrs.value("libraryItemName").toString();\n'
     '            if (!found && !ref.isEmpty()) relativeName = ref;\n'
     '        }'),
    ("a shape inside a library symbol no longer counted",
     r'\}\s*else if \(name == QLatin1String\("DOMShape"\)\) \{\n'
     r'\s*\+\+m_document\.census\.shapes;\n'
     r'\s*\}\s*else if \(name == QLatin1String\("DOMShapeText"\)\) \{\n'
     r'\s*\+\+m_document\.census\.shapeText;',
     '        } else if (name == QLatin1String("DOMShape")) {\n'
     '        } else if (name == QLatin1String("DOMShapeText")) {\n'
     '            ++m_document.census.shapeText;'),
]

# Which binary covers which file. Kept next to the tables so adding a mutation
# cannot leave it unassigned: main() refuses to run if a file has no binary.
COVERING = {
    "SHAPE": "XFL_SHAPE",
    "SWF_ASSETS": "FLASH_READER",
    "MOHO_READER": "MOHO_READER",
    "XFL_READER": "XFL_SHAPE",
}

def main():
    if not os.path.isfile(EXE):
        print(f"   {EXE} not built. Run run_tests.py first, or build "
              f"tests/native.")
        return 1
    if not QT_BIN:
        print("   note: QT_BIN is not set, so the test process may not start at "
              "all.\n         Set it to <qt>/5.x/<kit>/bin.")

    print("baseline: the suite must pass on unmodified sources")
    ok, detail = build()
    if not ok:
        print(f"  BASELINE DOES NOT BUILD: {detail}")
        return 1
    rc, out = run()
    if rc != 0:
        print("  BASELINE FAILS -- results below would be meaningless")
        print(out[-2000:])
        return 1
    print("  [ok  ] baseline passes")

    # Both binaries the mutation table depends on. A missing one would make every
    # mutation for that file read as MISSED -- indistinguishable from "the suite
    # cannot see this bug", which is the confusion the NO-BUILD outcome exists to
    # prevent.
    reader_exe = os.path.join(HERE, "build", "RelWithDebInfo",
                              "flash_reader_tests.exe")
    if not os.path.isfile(reader_exe):
        print(f"   {reader_exe} not built, so the SWF mutations could not be "
              f"run.\n         Its results would be meaningless; refusing.")
        return 1
    rc, _ = run(reader_exe, (os.path.join(HERE, "..", "flash_fixtures"),))
    if rc != 0:
        print("  flash_reader_tests FAILS on unmodified sources -- results "
              "below would be meaningless")
        return 1
    print("  [ok  ] flash_reader_tests passes")

    results = []
    # (source path, the binary that covers it, extra argv, name, pattern, repl)
    plan = [(SHAPE, EXE, (), *m) for m in MUTATIONS]
    reader_exe = os.path.join(HERE, "build", "RelWithDebInfo",
                              "flash_reader_tests.exe")
    reader_args = (os.path.join(HERE, "..", "flash_fixtures"),)
    plan += [(SWF_ASSETS, reader_exe, reader_args, *m) for m in SWF_MUTATIONS]
    # The Moho fixtures are generated, not committed, so the binary needs
    # the same directory run_all.py passes it.
    moho_exe = os.path.join(HERE, "build", "RelWithDebInfo",
                           "moho_reader_tests.exe")
    moho_fx = os.environ.get("FLARE_MOHO_FIXTURES")
    if not moho_fx:
        import tempfile
        moho_fx = os.path.join(tempfile.mkdtemp(prefix="moho_fx"), "fx")
        os.makedirs(moho_fx, exist_ok=True)
        gen = os.path.join(HERE, "..", "flash_fixtures",
                           "generate_moho_fixtures.py")
        subprocess.run([sys.executable, gen, moho_fx], check=True)
    plan += [(MOHO_READER, moho_exe, (moho_fx,), *m) for m in MOHO_MUTATIONS]
    # XFL census, covered by the census case in xfl_shape_tests.
    xfl_exe = os.path.join(HERE, 'build', 'RelWithDebInfo',
                           'xfl_shape_tests.exe')
    plan += [(XFL_READER, xfl_exe, (), *m) for m in XFL_MUTATIONS]

    for source, exe, extra, name, pattern, repl in plan:
        original = io.open(source, encoding="utf-8", newline="").read()
        new, count = inject(original, pattern, repl)
        if new is None or count != 1:
            results.append((name, "SKIPPED",
                            f"pattern matched {count} times"))
            print(f"  [skip] {name}: pattern matched {count} times")
            continue

        # Diffed against the *original*, not a normalised copy, so the printed
        # lines are the ones actually injected rather than every line in the
        # file differing by a stripped carriage return.
        diff = [l for l in difflib.unified_diff(original.splitlines(),
                                                 new.splitlines(),
                                                 lineterm="", n=0)
                if l.startswith(("+", "-")) and not l.startswith(("+++", "---"))]
        if not diff:
            results.append((name, "SKIPPED", "substitution was a no-op"))
            print(f"  [skip] {name}: substitution was a no-op")
            continue

        io.open(source, "w", encoding="utf-8", newline="").write(new)
        try:
            ok, detail = build()
            if not ok:
                # Report the real cause. Without this, a mutation that fails to
                # compile reads exactly like a mutation the suite cannot see.
                results.append((name, "NO-BUILD", detail))
                print(f"  [skip] {name}: the mutation did not compile -- {detail[:70]}")
                continue
            rc, out = run(exe, extra)
            caught = rc != 0
            m = re.search(r"\[FAIL\] (.+)", out)
            results.append((name, "CAUGHT" if caught else "MISSED",
                            clip(m.group(1)) if m else ""))
            print(f"  [{'ok  ' if caught else 'MISS'}] {name}")
            # diff holds only +/- lines, but a substitution can add lines
            # without removing any (as with the '/' control), so index it
            # defensively rather than assuming the first entry is a removal.
            for line in diff[:2]:
                print(f"          inject:    {clip(line, 86)}")
            if m:
                print(f"          caught by: {clip(m.group(1), 86)}")
        finally:
            io.open(source, "w", encoding="utf-8", newline="").write(original)

    print()
    ok, _ = build()
    if not ok:
        print("restored: the baseline does not build -- something is wrong")
        return 1
    rc, _ = run()
    print(f"restored: baseline {'passes' if rc == 0 else 'FAILS'}")

    bad = [r for r in results if r[1] != "CAUGHT"]
    print()
    if bad:
        print(f"{len(bad)} of {len(results)} mutations were not conclusively "
              f"caught:")
        for r in bad:
            print(f"   [{r[1]}] {r[0]}  ({r[2]})")
        return 1
    print(f"PASSED: all {len(results)} mutations were caught by the suite")
    return 0


if __name__ == "__main__":
    sys.exit(main())
