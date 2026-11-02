//! Port of tnzbase/permissionsmanager.cpp. Same logic: users keyed by name,
//! per-user SVN credential lists, current user falls back to "guest".
//! C ABI at the bottom replaces the C++ Imp; include/permissionsmanager.h calls it.
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::sync::OnceLock;

struct User {
    svn_usernames: Vec<String>,
    svn_passwords: Vec<String>,
}

impl User {
    fn new() -> Self {
        User { svn_usernames: vec![], svn_passwords: vec![] }
    }
    fn svn_username(&self, index: i32) -> String {
        usize::try_from(index).ok().and_then(|i| self.svn_usernames.get(i)).cloned().unwrap_or_default()
    }
    fn svn_password(&self, index: i32) -> String {
        usize::try_from(index).ok().and_then(|i| self.svn_passwords.get(i)).cloned().unwrap_or_default()
    }
}

pub struct PermissionsManager {
    users: BTreeMap<String, User>,
    user: Option<String>,
    /// permissions.xml location. Passwords are NOT kept in memory: they are read
    /// from this file only when asked for and dropped again right away.
    file: std::path::PathBuf,
}

impl PermissionsManager {
    /// `config_dir` = TEnv::getConfigDir(); `user_name` = TSystem::getUserName().
    pub fn new(config_dir: &Path, user_name: &str) -> Self {
        let file = config_dir.join("permissions.xml");
        let mut m = PermissionsManager { users: BTreeMap::new(), user: None, file };
        m.load_permissions(&m.file.clone());
        for u in m.users.values_mut() {
            u.svn_passwords.clear(); // never retained after load
        }
        m.user = if m.users.contains_key(user_name) {
            Some(user_name.to_owned())
        } else if m.users.contains_key("guest") {
            Some("guest".to_owned())
        } else {
            None
        };
        m
    }

    fn find_user_mut(&mut self, name: &str) -> &mut User {
        self.users.entry(name.to_owned()).or_insert_with(User::new)
    }

    /// Mirrors loadPermissions(): any unexpected tag stops loading, keeping what was read.
    fn load_permissions(&mut self, fp: &Path) {
        let Ok(text) = std::fs::read_to_string(fp) else { return };
        let Ok(doc) = roxmltree::Document::parse(&text) else { return };
        let root = doc.root_element();
        if root.tag_name().name() != "permissions" {
            return;
        }
        for tag in root.children().filter(|n| n.is_element()) {
            match tag.tag_name().name() {
                "users" => {
                    for u in tag.children().filter(|n| n.is_element()) {
                        if u.tag_name().name() != "user" {
                            return;
                        }
                        let name = u.attribute("name").unwrap_or("");
                        if name.is_empty() {
                            return;
                        }
                        self.find_user_mut(name);
                        for t in u.children().filter(|n| n.is_element()) {
                            match t.tag_name().name() {
                                "roles" => {} // no longer used
                                "svn" => {
                                    let user = self.find_user_mut(name);
                                    user.svn_usernames.push(t.attribute("name").unwrap_or("").to_owned());
                                    user.svn_passwords.push(t.attribute("password").unwrap_or("").to_owned());
                                }
                                _ => return,
                            }
                        }
                    }
                }
                "roles" => {} // no longer used
                _ => return,
            }
        }
    }

    pub fn svn_user_name(&self, index: i32) -> String {
        self.user.as_ref().and_then(|u| self.users.get(u)).map(|u| u.svn_username(index)).unwrap_or_default()
    }

    pub fn svn_password(&self, index: i32) -> String {
        let Some(name) = self.user.as_ref() else { return String::new() };
        // Re-read the file into a temporary manager; the passwords live only in
        // that local value and are dropped when this function returns.
        let mut tmp = PermissionsManager { users: BTreeMap::new(), user: None, file: self.file.clone() };
        tmp.load_permissions(&self.file);
        tmp.users.get(name).map(|u| u.svn_password(index)).unwrap_or_default()
    }
}

static INSTANCE: OnceLock<PermissionsManager> = OnceLock::new();

unsafe fn cstr(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

/// Idempotent singleton init (PermissionsManager::instance()).
#[no_mangle]
pub unsafe extern "C" fn flare_permissions_init(config_dir: *const c_char, user_name: *const c_char) {
    let (c, u) = (cstr(config_dir), cstr(user_name));
    INSTANCE.get_or_init(|| PermissionsManager::new(Path::new(&c), &u));
}

fn out(s: String) -> *mut c_char {
    CString::new(s).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Free with flare_permissions_free.
#[no_mangle]
pub extern "C" fn flare_permissions_svn_username(index: i32) -> *mut c_char {
    out(INSTANCE.get().map(|m| m.svn_user_name(index)).unwrap_or_default())
}

#[no_mangle]
pub extern "C" fn flare_permissions_svn_password(index: i32) -> *mut c_char {
    out(INSTANCE.get().map(|m| m.svn_password(index)).unwrap_or_default())
}

#[no_mangle]
pub unsafe extern "C" fn flare_permissions_free(p: *mut c_char) {
    if !p.is_null() {
        drop(CString::from_raw(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(xml: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!(
            "flare_perm_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("permissions.xml"), xml).unwrap();
        d
    }

    #[test]
    fn loads_user_and_falls_back_to_guest() {
        let d = dir_with(r#"<permissions><users><user name="guest"><roles/><svn name="a" password="p"/><svn name="b" password="q"/></user></users><roles/></permissions>"#);
        let m = PermissionsManager::new(&d, "nobody");
        assert_eq!(m.svn_user_name(1), "b");
        assert_eq!(m.svn_password(0), "p");
        assert_eq!(m.svn_user_name(2), "");
        assert_eq!(m.svn_user_name(-1), "");
    }

    #[test]
    fn missing_file_gives_empty() {
        let m = PermissionsManager::new(Path::new("/nonexistent_flare"), "x");
        assert_eq!(m.svn_user_name(0), "");
    }
}
