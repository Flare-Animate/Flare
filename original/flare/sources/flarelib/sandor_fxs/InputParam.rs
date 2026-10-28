//! Port of `flarelib/sandor_fxs/InputParam.h` / `InputParam.cpp`:
//! the input parameters for a sandor fx (scale + whether to read an external
//! config file).

#[derive(Clone, Debug, PartialEq)]
pub struct CInputParam {
    pub m_scale: f64,
    pub m_is_econf: bool,
    pub m_econf_fn: String,
}

impl CInputParam {
    pub fn new() -> Self {
        Self {
            m_scale: 0.0,
            m_is_econf: false,
            m_econf_fn: String::new(),
        }
    }
}

impl Default for CInputParam {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_cpp() {
        let p = CInputParam::new();
        assert_eq!(p.m_scale, 0.0);
        assert!(!p.m_is_econf);
        assert!(p.m_econf_fn.is_empty());
    }

    #[test]
    fn clone_matches_cpp() {
        let p = CInputParam {
            m_scale: 2.5,
            m_is_econf: true,
            m_econf_fn: "foo.econf".into(),
        };
        let q = p.clone();
        assert_eq!(p, q);
    }
}