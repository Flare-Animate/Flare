//! Port of TEnv::getSystemVarPathSetValue split logic (flare/sources/common/tapptools/tenv.cpp).
//! Split on ';'; a trailing ';' adds no empty entry; empty value -> empty list.

pub fn path_set(value: &str) -> Vec<String> {
    if value.is_empty() {
        return vec![];
    }
    let mut parts: Vec<String> = value.split(';').map(String::from).collect();
    if value.ends_with(';') {
        parts.pop();
    }
    parts
}


// ---------------------------------------------------------------------------
// FLAREROOT / "stuff" directory discovery (issues #110, #112, #114, #116).
//
// Pure function: every input (environment, cwd, executable dir, probe for
// "does this directory exist") is passed in so the whole decision table is unit
// testable and the function can never abort. C++ (tenv.cpp) gathers the inputs
// and calls it through `flare_tenv_resolve_stuff_dir` (flare_formats.rs).
// ---------------------------------------------------------------------------
use std::path::{Path, PathBuf};

#[derive(Default, Clone, Debug)]
pub struct StuffInputs {
    /// Value of the FLAREROOT environment variable, if set.
    pub env_root: Option<String>,
    /// Root stored in SystemVar.ini (per-user config), if any.
    pub ini_root: Option<String>,
    /// Command-line `-FLAREROOT` override, if any.
    pub arg_root: Option<String>,
    pub cwd: PathBuf,
    pub exe_dir: Option<PathBuf>,
    /// $APPDIR (AppImage mount point), if set.
    pub appdir: Option<String>,
    pub is_macos: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StuffResolution {
    pub path: Option<PathBuf>,
    /// True when the dir is a bundled/portable one (portablestuff/tahomastuff).
    pub portable: bool,
    /// Human-readable decision trace, one line per candidate tried.
    pub trace: Vec<String>,
}

fn nonempty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Resolve `p` against `cwd` when relative.
fn absolutize(p: &str, cwd: &Path) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        cwd.join(pb)
    }
}

pub fn resolve_stuff_dir(inp: &StuffInputs, is_dir: &dyn Fn(&Path) -> bool) -> StuffResolution {
    let mut trace = Vec::new();
    let try_dir = |label: &str, p: PathBuf, trace: &mut Vec<String>| -> Option<PathBuf> {
        let ok = is_dir(&p);
        trace.push(format!("{}: {} -> {}", label, p.display(), if ok { "found" } else { "missing" }));
        if ok {
            Some(p)
        } else {
            None
        }
    };
    let done = |path: PathBuf, portable: bool, trace: Vec<String>| StuffResolution {
        path: Some(path),
        portable,
        trace,
    };

    // 1. Explicit overrides: command line, then environment. Relative values
    //    are taken relative to the working directory. A bad value is skipped
    //    (with a trace line) instead of being fatal.
    if let Some(v) = nonempty(&inp.arg_root) {
        if let Some(p) = try_dir("command-line FLAREROOT", absolutize(v, &inp.cwd), &mut trace) {
            return done(p, false, trace);
        }
    } else {
        trace.push("command-line FLAREROOT: not set".into());
    }
    // 2. Portable layouts: next to the working dir, then next to the executable
    //    (Windows portable zip, AppImage usr/bin), then the macOS bundle.
    const PORTABLE: [&str; 2] = ["portablestuff", "tahomastuff"];
    let mut bases: Vec<(String, PathBuf)> = vec![("cwd".into(), inp.cwd.clone())];
    if let Some(e) = &inp.exe_dir {
        bases.push(("exe dir".into(), e.clone()));
    }
    if let Some(a) = nonempty(&inp.appdir) {
        bases.push(("APPDIR/usr/bin".into(), Path::new(a).join("usr").join("bin")));
    }
    for (name, base) in &bases {
        for leaf in PORTABLE {
            if let Some(p) = try_dir(&format!("portable ({})", name), base.join(leaf), &mut trace) {
                return done(p, true, trace);
            }
        }
    }
    if inp.is_macos {
        // Flare.app/Contents/MacOS/Flare -> Flare.app/Contents/Resources
        let mut res: Vec<PathBuf> = vec![inp.cwd.join("Contents").join("Resources")];
        if let Some(e) = &inp.exe_dir {
            res.push(e.join("..").join("Resources"));
        }
        for r in res {
            for leaf in ["tahomastuff", "portablestuff"] {
                if let Some(p) = try_dir("macOS bundle Resources", r.join(leaf), &mut trace) {
                    return done(p, true, trace);
                }
            }
        }
    }

    // 2b. Environment FLAREROOT: only when no portable install was found above
    //     (a portable install wins over a stray env var).
    if let Some(v) = nonempty(&inp.env_root) {
        if let Some(p) = try_dir("env FLAREROOT", absolutize(v, &inp.cwd), &mut trace) {
            return done(p, false, trace);
        }
    } else {
        trace.push("env FLAREROOT: not set".into());
    }

    // 3. Per-user root recorded in SystemVar.ini (seeded on first run).
    if let Some(v) = nonempty(&inp.ini_root) {
        // An explicitly configured root that is gone is an error the user must
        // see ("folder not found"); do not silently use an installed layout.
        let p = try_dir("SystemVar.ini root", absolutize(v, &inp.cwd), &mut trace);
        if let Some(p) = p {
            return done(p, false, trace);
        }
        trace.push("SystemVar.ini root is set but missing: not falling back to installed layouts".into());
        return StuffResolution { path: None, portable: false, trace };
    } else {
        trace.push("SystemVar.ini root: not set".into());
    }

    // 4. Installed layouts. AppImage mount first (the fix for #112/#114/#116:
    //    the old code only looked relative to the process, so an AppImage run
    //    from an arbitrary cwd found nothing and the startup path aborted).
    let mut installed: Vec<(String, PathBuf)> = Vec::new();
    if let Some(a) = nonempty(&inp.appdir) {
        let a = Path::new(a);
        installed.push(("APPDIR share".into(), a.join("usr/share/flare/stuff")));
        installed.push(("APPDIR share (legacy)".into(), a.join("usr/share/flare/portablestuff")));
    }
    if let Some(e) = &inp.exe_dir {
        installed.push(("exe dir ../share".into(), e.join("../share/flare/stuff")));
    }
    for fixed in ["/usr/local/share/flare/stuff", "/usr/share/flare/stuff", "/opt/flare/share/flare/stuff"] {
        installed.push(("system share".into(), PathBuf::from(fixed)));
    }
    for (label, p) in installed {
        if let Some(p) = try_dir(&label, p, &mut trace) {
            return done(p, false, trace);
        }
    }

    trace.push("no stuff directory found".into());
    StuffResolution { path: None, portable: false, trace }
}

/// Real filesystem probe used by the FFI wrapper.
pub fn fs_is_dir(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

#[cfg(test)]
mod t {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn split() {
        assert_eq!(path_set(""), Vec::<String>::new());
        assert_eq!(path_set("a"), vec!["a"]);
        assert_eq!(path_set("a;b"), vec!["a", "b"]);
        assert_eq!(path_set("a;"), vec!["a"]);
        assert_eq!(path_set("a;;"), vec!["a", ""]);
        assert_eq!(path_set(";"), vec![""]);
        assert_eq!(path_set(";a"), vec!["", "a"]);
    }

    fn fs(dirs: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = dirs.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }
    fn base() -> StuffInputs {
        StuffInputs { cwd: PathBuf::from("/work"), ..Default::default() }
    }

    #[test]
    fn env_var_set_and_valid() {
        let mut i = base();
        i.env_root = Some("/opt/stuff".into());
        let r = resolve_stuff_dir(&i, &fs(&["/opt/stuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/opt/stuff")));
        assert!(!r.portable);
    }

    #[test]
    fn env_var_unset_or_blank_is_ignored() {
        let mut i = base();
        i.env_root = Some("   ".into());
        let r = resolve_stuff_dir(&i, &fs(&[]));
        assert_eq!(r.path, None);
        assert!(r.trace.iter().any(|l| l.contains("env FLAREROOT: not set")));
        i.env_root = None;
        assert_eq!(resolve_stuff_dir(&i, &fs(&[])).path, None);
    }

    #[test]
    fn env_var_pointing_nowhere_falls_through_not_aborts() {
        let mut i = base();
        i.env_root = Some("/does/not/exist".into());
        let r = resolve_stuff_dir(&i, &fs(&["/usr/share/flare/stuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/usr/share/flare/stuff")));
        assert!(r.trace.iter().any(|l| l.contains("/does/not/exist") && l.contains("missing")));
    }

    #[test]
    fn relative_env_path_resolves_against_cwd() {
        let mut i = base();
        i.env_root = Some("rel/stuff".into());
        let r = resolve_stuff_dir(&i, &fs(&["/work/rel/stuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/work/rel/stuff")));
    }

    #[test]
    fn command_line_beats_env() {
        let mut i = base();
        i.arg_root = Some("/a".into());
        i.env_root = Some("/b".into());
        let r = resolve_stuff_dir(&i, &fs(&["/a", "/b"]));
        assert_eq!(r.path, Some(PathBuf::from("/a")));
    }

    #[test]
    fn appimage_mount_layout_portablestuff_in_usr_bin() {
        // AppImage: cwd is arbitrary (~/Downloads), exe in the squashfs mount.
        let mut i = base();
        i.exe_dir = Some(PathBuf::from("/tmp/.mount_Flare-x/usr/bin"));
        i.appdir = Some("/tmp/.mount_Flare-x".into());
        let r = resolve_stuff_dir(&i, &fs(&["/tmp/.mount_Flare-x/usr/bin/portablestuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/tmp/.mount_Flare-x/usr/bin/portablestuff")));
        assert!(r.portable);
    }

    #[test]
    fn appimage_mount_layout_share_stuff_when_exe_dir_unknown() {
        let mut i = base();
        i.appdir = Some("/tmp/.mount_Flare-x".into());
        let r = resolve_stuff_dir(&i, &fs(&["/tmp/.mount_Flare-x/usr/share/flare/stuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/tmp/.mount_Flare-x/usr/share/flare/stuff")));
        assert!(!r.portable);
    }

    #[test]
    fn portable_next_to_executable_with_foreign_cwd() {
        let mut i = base();
        i.exe_dir = Some(PathBuf::from("C:/Flare"));
        let r = resolve_stuff_dir(&i, &fs(&["C:/Flare/portablestuff"]));
        assert_eq!(r.path, Some(PathBuf::from("C:/Flare/portablestuff")));
        assert!(r.portable);
        // legacy name still accepted
        let r = resolve_stuff_dir(&i, &fs(&["C:/Flare/tahomastuff"]));
        assert!(r.portable && r.path.is_some());
    }

    #[test]
    fn portable_in_cwd() {
        let r = resolve_stuff_dir(&base(), &fs(&["/work/portablestuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/work/portablestuff")));
    }

    #[test]
    fn macos_bundle_resources() {
        let mut i = base();
        i.is_macos = true;
        i.exe_dir = Some(PathBuf::from("/Applications/Flare.app/Contents/MacOS"));
        let r = resolve_stuff_dir(
            &i,
            &fs(&["/Applications/Flare.app/Contents/MacOS/../Resources/portablestuff"]),
        );
        assert!(r.path.is_some() && r.portable);
        // plain "stuff" in Resources is NOT portable
        assert_eq!(
            resolve_stuff_dir(&i, &fs(&["/Applications/Flare.app/Contents/MacOS/../Resources/stuff"])).path,
            None
        );
        // not consulted on other platforms
        i.is_macos = false;
        assert_eq!(
            resolve_stuff_dir(&i, &fs(&["/Applications/Flare.app/Contents/MacOS/../Resources/stuff"])).path,
            None
        );
    }

    #[test]
    fn ini_root_missing_does_not_fall_through_to_installed() {
        let mut i = base();
        i.ini_root = Some("/gone/stuff".into());
        let r = resolve_stuff_dir(&i, &fs(&["/usr/share/flare/stuff"]));
        assert_eq!(r.path, None);
        assert!(r.trace.iter().any(|l| l.contains("/gone/stuff") && l.contains("missing")));
        // unset ini root still falls through
        i.ini_root = None;
        assert!(resolve_stuff_dir(&i, &fs(&["/usr/share/flare/stuff"])).path.is_some());
    }

    #[test]
    fn portable_beats_env_flareroot() {
        let mut i = base();
        i.env_root = Some("/opt/stuff".into());
        let r = resolve_stuff_dir(&i, &fs(&["/opt/stuff", "/work/portablestuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/work/portablestuff")));
        assert!(r.portable);
    }

    #[test]
    fn ini_root_used_when_nothing_better() {
        let mut i = base();
        i.ini_root = Some("/home/u/.config/Flare/stuff".into());
        let r = resolve_stuff_dir(&i, &fs(&["/home/u/.config/Flare/stuff"]));
        assert_eq!(r.path, Some(PathBuf::from("/home/u/.config/Flare/stuff")));
    }

    #[test]
    fn installed_share_next_to_prefix() {
        let mut i = base();
        i.exe_dir = Some(PathBuf::from("/opt/flare/bin"));
        let r = resolve_stuff_dir(&i, &fs(&["/opt/flare/bin/../share/flare/stuff"]));
        assert!(r.path.is_some() && !r.portable);
    }

    #[test]
    fn missing_everything_returns_none_with_trace() {
        let r = resolve_stuff_dir(&base(), &fs(&[]));
        assert_eq!(r.path, None);
        assert!(r.trace.last().unwrap().contains("no stuff directory"));
        assert!(r.trace.len() > 3);
    }

    #[test]
    fn weird_inputs_never_panic() {
        let mut i = base();
        i.env_root = Some("\0\u{feff}".into());
        i.appdir = Some(String::new());
        i.exe_dir = Some(PathBuf::new());
        let _ = resolve_stuff_dir(&i, &fs(&[]));
        let _ = resolve_stuff_dir(&StuffInputs::default(), &fs(&[]));
    }
}
