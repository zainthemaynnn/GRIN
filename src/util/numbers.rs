/// Multiplying by zero makes things weird, since after being hit by zero,
/// numbers can't be multiplied back again. Instead of using a raw `f32` value,
/// these sorts of numbers are calculated off of a mulstack.
#[derive(Default)]
pub struct MulStack {
    pub multipliers: Vec<f32>,
}

impl MulStack {
    /// Adds a multiplier to the mulstack.
    pub fn add(&mut self, scale: f32) {
        self.multipliers.push(scale);
    }

    /// Removes a multiplier from the mulstack. `MulStackError::BadUnscale` if not found.
    pub fn remove(&mut self, scale: f32) -> Result<(), MulStackError> {
        match self.multipliers.iter().position(|v| *v == scale) {
            Some(i) => {
                self.multipliers.remove(i);
                Ok(())
            }
            None => Err(MulStackError::BadUnscale(scale)),
        }
    }
}

// imagine only impl'ing From for the reference. couldn't be me.
impl From<&MulStack> for f32 {
    fn from(value: &MulStack) -> Self {
        value.multipliers.iter().fold(1.0, |acc, scale| acc * scale)
    }
}

impl From<Vec<f32>> for MulStack {
    fn from(value: Vec<f32>) -> Self {
        Self { multipliers: value }
    }
}

#[derive(Debug)]
pub enum MulStackError {
    BadUnscale(f32),
}

impl std::fmt::Display for MulStackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadUnscale(scale) => f.write_fmt(format_args!(
                "`MulStack::remove` with unidentified multiplier {}",
                scale,
            )),
        }
    }
}
