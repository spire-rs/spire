use crate::{Error, Result, UsageTotals};

/// Aggregate input and output token limits for model calls.
///
/// The default has no limits. Caps use settled provider usage; in-flight
/// calls may cross them, and no new calls start once either cap is reached.
#[derive(Debug, Clone, Copy, Default)]
pub struct Budget {
    pub(crate) input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
}

impl Budget {
    /// Caps aggregate input and output tokens independently.
    #[must_use]
    pub const fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
        }
    }

    /// Sets only the aggregate input cap, preserving the output cap.
    #[must_use]
    pub const fn input_tokens(mut self, max: u64) -> Self {
        self.input_tokens = Some(max);
        self
    }

    /// Sets only the aggregate output cap, preserving the input cap.
    #[must_use]
    pub const fn output_tokens(mut self, max: u64) -> Self {
        self.output_tokens = Some(max);
        self
    }

    pub(crate) fn check(
        self,
        usage: &UsageTotals,
        unknown_input: bool,
        unknown_output: bool,
    ) -> Result<()> {
        for (limit, consumed, unknown, direction, reason) in [
            (
                self.input_tokens,
                usage.input_tokens,
                unknown_input,
                "input",
                "input tokens",
            ),
            (
                self.output_tokens,
                usage.output_tokens,
                unknown_output,
                "output",
                "output tokens",
            ),
        ] {
            if let Some(max) = limit {
                if unknown {
                    return Err(Error::Configuration(format!(
                        "{direction} token budget requires reported {direction} usage"
                    )));
                }
                if consumed >= max {
                    return Err(Error::Budget(reason));
                }
            }
        }
        Ok(())
    }

    pub(crate) const fn is_limited(self) -> bool {
        self.input_tokens.is_some() || self.output_tokens.is_some()
    }
}
