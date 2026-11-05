//! Parameter sets for effects, ported from the old `CParams` / `CInputParam` pair: a list of
//! effect parameters plus one scale that stretches them all, so an effect tuned for one
//! resolution renders at another.

/// Most parameters one set holds; an effect is a handful, so this is only a bound on input.
pub const MAX_PARAMS: usize = 1_024;

/// Why a parameter set could not be built or scaled.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FxParamsError {
    /// The scale is NaN, infinite or not above zero, which would collapse or poison every parameter.
    #[error("scale must be finite and above zero")]
    BadScale,
    /// Reading a list of parameters from an effect-configuration file was never finished in the
    /// original (the call was commented out), so a request for it is refused by name.
    #[error("loading effect parameters from an effect-configuration file is not supported")]
    EconfUnsupported,
    /// The set would hold more than [`MAX_PARAMS`] entries.
    #[error("too many effect parameters (limit {MAX_PARAMS})")]
    TooMany,
}

/// A parameter that can be stretched by a scale factor (lengths, radii, offsets).
pub trait Scalable {
    /// Multiplies the parameter's size-dependent values by `factor` (already validated).
    fn scale(&mut self, factor: f64);
}

/// What the caller asked for: the scale, and whether the parameters come from a configuration file.
#[derive(Debug, Clone, PartialEq)]
pub struct InputParam {
    pub scale: f64,
    pub is_econf: bool,
    pub econf_file_name: String,
}

impl Default for InputParam {
    /// The original defaulted the scale to 0, which is not valid; here it is 1 (no stretch).
    fn default() -> Self {
        Self {
            scale: 1.0,
            is_econf: false,
            econf_file_name: String::new(),
        }
    }
}

/// A list of effect parameters and the scale that applies to them.
#[derive(Debug, Clone, PartialEq)]
pub struct Params<P> {
    params: Vec<P>,
    scale: f64,
}

impl<P> Default for Params<P> {
    fn default() -> Self {
        Self {
            params: Vec::new(),
            scale: 1.0,
        }
    }
}

fn check_scale(scale: f64) -> Result<f64, FxParamsError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(FxParamsError::BadScale)
    }
}

impl<P: Scalable> Params<P> {
    /// Builds a set from a request and the one parameter it describes (the only source the
    /// original supported).
    pub fn from_input(input: &InputParam, param: P) -> Result<Self, FxParamsError> {
        if input.is_econf {
            return Err(FxParamsError::EconfUnsupported);
        }
        Ok(Self {
            params: vec![param],
            scale: check_scale(input.scale)?,
        })
    }

    /// Adds a parameter, refusing past [`MAX_PARAMS`].
    pub fn push(&mut self, param: P) -> Result<(), FxParamsError> {
        if self.params.len() >= MAX_PARAMS {
            return Err(FxParamsError::TooMany);
        }
        self.params.push(param);
        Ok(())
    }

    pub fn params(&self) -> &[P] {
        &self.params
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale
    }

    /// Scales every parameter by `factor`. Nothing changes when the factor is refused.
    pub fn scale_by(&mut self, factor: f64) -> Result<(), FxParamsError> {
        let factor = check_scale(factor)?;
        self.params.iter_mut().for_each(|p| p.scale(factor));
        Ok(())
    }

    /// Scales every parameter by the set's own scale.
    pub fn apply_scale(&mut self) {
        let factor = self.scale;
        self.params.iter_mut().for_each(|p| p.scale(factor));
    }
}

#[cfg(test)]
mod tests;
