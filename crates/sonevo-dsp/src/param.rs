//! A device's setting described: its range, its default, how it is shown and automated.

/// How a setting goes from its least to its most, as a knob or an automation moves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Linear,
    /// Each step of the knob multiplies: frequencies, times.
    Log,
    /// One of the choices, by its index.
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Param {
    pub id: &'static str,
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// `Hz`, `dB`, `ms`, `%`… as shown after the value.
    pub unit: &'static str,
    pub scale: Scale,
}

impl Param {
    pub const fn linear(
        id: &'static str,
        name: &'static str,
        min: f64,
        max: f64,
        default: f64,
        unit: &'static str,
    ) -> Param {
        Param { id, name, min, max, default, unit, scale: Scale::Linear }
    }

    pub const fn log(
        id: &'static str,
        name: &'static str,
        min: f64,
        max: f64,
        default: f64,
        unit: &'static str,
    ) -> Param {
        Param { id, name, min, max, default, unit, scale: Scale::Log }
    }

    pub const fn choice(
        id: &'static str,
        name: &'static str,
        choices: &'static [&'static str],
        default: usize,
    ) -> Param {
        Param {
            id,
            name,
            min: 0.0,
            max: (choices.len() - 1) as f64,
            default: default as f64,
            unit: "",
            scale: Scale::Choice(choices),
        }
    }

    /// The value at `position`, from 0 to 1 along the knob.
    pub fn from_normal(&self, position: f64) -> f64 {
        let p = position.clamp(0.0, 1.0);
        match self.scale {
            Scale::Linear => self.min + p * (self.max - self.min),
            Scale::Log => self.min * (self.max / self.min).powf(p),
            Scale::Choice(_) => (self.min + p * (self.max - self.min)).round(),
        }
    }

    /// Where `value` is along the knob, from 0 to 1.
    pub fn to_normal(&self, value: f64) -> f64 {
        let v = value.clamp(self.min, self.max);
        let p = match self.scale {
            Scale::Linear | Scale::Choice(_) => (v - self.min) / (self.max - self.min),
            Scale::Log => (v / self.min).ln() / (self.max / self.min).ln(),
        };
        if p.is_finite() {
            p
        } else {
            0.0
        }
    }

    pub fn clamp(&self, value: f64) -> f64 {
        let v = value.clamp(self.min, self.max);
        match self.scale {
            Scale::Choice(_) => v.round(),
            _ => v,
        }
    }
}
