//! Certified bounded-word dyadic interval arithmetic for the flow IPM.
//!
//! CKLPPS22 Section 3 requires fixed-point words with polylogarithmically
//! bounded bit length. Equation (9) additionally needs `log(x)` and
//! `x^-alpha`. This module encloses those values using dyadic intervals,
//! outward-rounded arithmetic, and Taylor-series remainder bounds. It does not
//! yet implement Equation (9), Definition 4.2, or an IPM iteration.

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};
use thiserror::Error;

/// Checked precision and source-model word bound for one computation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedPointConfig {
    pub input_encoding_bits: u64,
    pub fractional_bits: u32,
    pub series_terms: u32,
    pub word_log_exponent: u32,
    pub maximum_word_bits: u64,
}

impl FixedPointConfig {
    /// Constructs a word bound of `ceil(log2(z + 1))^exponent` bits for an
    /// instance encoded with `z` bits.
    ///
    /// # Errors
    ///
    /// Returns an error for zero precision, too few series terms, a zero
    /// exponent, arithmetic overflow, or a bound too small to store one fixed
    /// point word.
    pub fn source_bounded(
        input_encoding_bits: u64,
        fractional_bits: u32,
        series_terms: u32,
        word_log_exponent: u32,
    ) -> Result<Self, FixedPointError> {
        if input_encoding_bits == 0
            || fractional_bits == 0
            || series_terms < 4
            || word_log_exponent == 0
        {
            return Err(FixedPointError::InvalidConfig);
        }
        let logarithmic_base = if input_encoding_bits == u64::MAX {
            u64::BITS.into()
        } else {
            ceil_log2(input_encoding_bits + 1)
        }
        .max(2);
        let maximum_word_bits = logarithmic_base
            .checked_pow(word_log_exponent)
            .ok_or(FixedPointError::InvalidConfig)?;
        if u64::from(fractional_bits) + 2 > maximum_word_bits {
            return Err(FixedPointError::InvalidConfig);
        }
        Ok(Self {
            input_encoding_bits,
            fractional_bits,
            series_terms,
            word_log_exponent,
            maximum_word_bits,
        })
    }
}

/// One closed interval whose endpoints are integer multiples of `2^-p`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DyadicInterval {
    lower_scaled: BigInt,
    upper_scaled: BigInt,
    fractional_bits: u32,
}

impl DyadicInterval {
    #[must_use]
    pub const fn fractional_bits(&self) -> u32 {
        self.fractional_bits
    }

    #[must_use]
    pub fn lower_scaled(&self) -> &BigInt {
        &self.lower_scaled
    }

    #[must_use]
    pub fn upper_scaled(&self) -> &BigInt {
        &self.upper_scaled
    }

    /// Returns whether the interval contains the supplied exact ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive denominator.
    pub fn contains_ratio(
        &self,
        numerator: i128,
        denominator: i128,
    ) -> Result<bool, FixedPointError> {
        if denominator <= 0 {
            return Err(FixedPointError::InvalidRatio);
        }
        let scale = BigInt::one() << self.fractional_bits;
        let scaled_numerator = BigInt::from(numerator) * scale;
        let denominator = BigInt::from(denominator);
        Ok(self.lower_scaled.clone() * &denominator <= scaled_numerator
            && scaled_numerator <= self.upper_scaled.clone() * denominator)
    }

    /// Returns whether the interval width is at most the supplied ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive numerator or denominator.
    pub fn width_at_most(
        &self,
        numerator: i128,
        denominator: i128,
    ) -> Result<bool, FixedPointError> {
        if numerator <= 0 || denominator <= 0 {
            return Err(FixedPointError::InvalidRatio);
        }
        let width = &self.upper_scaled - &self.lower_scaled;
        let scale = BigInt::one() << self.fractional_bits;
        Ok(width * BigInt::from(denominator) <= BigInt::from(numerator) * scale)
    }

    /// Returns whether this interval intersects an exact rational interval.
    ///
    /// # Errors
    ///
    /// Returns an error for reversed bounds or a nonpositive denominator.
    pub fn overlaps_ratio_interval(
        &self,
        lower_numerator: i128,
        upper_numerator: i128,
        denominator: i128,
    ) -> Result<bool, FixedPointError> {
        if lower_numerator > upper_numerator || denominator <= 0 {
            return Err(FixedPointError::InvalidRatio);
        }
        let scale = BigInt::one() << self.fractional_bits;
        let denominator = BigInt::from(denominator);
        let external_lower = BigInt::from(lower_numerator) * &scale;
        let external_upper = BigInt::from(upper_numerator) * scale;
        Ok(self.lower_scaled.clone() * &denominator <= external_upper
            && external_lower <= self.upper_scaled.clone() * denominator)
    }

    #[must_use]
    pub fn is_strictly_positive(&self) -> bool {
        self.lower_scaled.is_positive()
    }

    #[must_use]
    pub fn absolute_upper_scaled(&self) -> BigInt {
        self.lower_scaled.abs().max(self.upper_scaled.abs())
    }
}

/// Auditable arithmetic work and bounded-word observations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FixedPointMetrics {
    pub arithmetic_operations: u64,
    pub outward_rounds: u64,
    pub logarithm_series_terms: u64,
    pub exponential_series_terms: u64,
    pub maximum_observed_word_bits: u64,
}

/// Certified dyadic arithmetic engine with an explicit source-model bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedFixedPoint {
    config: FixedPointConfig,
    metrics: FixedPointMetrics,
    scale: BigInt,
}

impl CertifiedFixedPoint {
    /// Creates an engine after validating its fixed-point configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured scale exceeds the word bound.
    pub fn new(config: FixedPointConfig) -> Result<Self, FixedPointError> {
        let scale = BigInt::one() << config.fractional_bits;
        let mut result = Self {
            config,
            metrics: FixedPointMetrics::default(),
            scale,
        };
        let scale = result.scale.clone();
        result.observe(&scale)?;
        Ok(result)
    }

    #[must_use]
    pub const fn config(&self) -> FixedPointConfig {
        self.config
    }

    #[must_use]
    pub const fn metrics(&self) -> FixedPointMetrics {
        self.metrics
    }

    /// Encloses an exact ratio in the configured dyadic grid.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive denominator or a word-bound breach.
    pub fn enclose_ratio(
        &mut self,
        numerator: i128,
        denominator: i128,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.enclose_big_ratio(&BigInt::from(numerator), &BigInt::from(denominator))
    }

    /// Encloses an arbitrary-precision exact ratio in the configured dyadic
    /// grid.
    ///
    /// # Errors
    ///
    /// Returns an error for a nonpositive denominator or a word-bound breach.
    pub fn enclose_big_ratio(
        &mut self,
        numerator: &BigInt,
        denominator: &BigInt,
    ) -> Result<DyadicInterval, FixedPointError> {
        if !denominator.is_positive() {
            return Err(FixedPointError::InvalidRatio);
        }
        self.ratio_interval(numerator.clone(), denominator)
    }

    /// Returns a certified enclosure of the natural logarithm.
    ///
    /// The enclosure uses `log(y) = 2 atanh((y-1)/(y+1))` after power-of-two
    /// range reduction. The omitted positive series tail is bounded by a
    /// geometric majorant.
    ///
    /// # Errors
    ///
    /// Returns an error unless the entire input interval is positive, or when
    /// the configured word/precision bound cannot certify the result.
    pub fn logarithm(&mut self, input: &DyadicInterval) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(input)?;
        if !input.lower_scaled.is_positive() {
            return Err(FixedPointError::NonpositiveInput);
        }
        let lower = self.logarithm_point(&input.lower_scaled)?;
        let upper = self.logarithm_point(&input.upper_scaled)?;
        self.interval(lower.lower_scaled, upper.upper_scaled)
    }

    /// Returns a certified enclosure of `exp(input)`.
    ///
    /// # Errors
    ///
    /// Returns an error when the interval precision or word bound cannot
    /// certify a strictly positive enclosure.
    pub fn exponential(
        &mut self,
        input: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(input)?;
        let lower = self.exponential_point(&input.lower_scaled)?;
        let upper = self.exponential_point(&input.upper_scaled)?;
        self.interval(lower.lower_scaled, upper.upper_scaled)
    }

    /// Returns a certified enclosure of `base^-alpha`.
    ///
    /// # Errors
    ///
    /// Returns an error unless `base` and `alpha` are wholly positive, or when
    /// the configured precision cannot certify the composed operation.
    pub fn negative_power(
        &mut self,
        base: &DyadicInterval,
        alpha: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(base)?;
        self.validate_interval(alpha)?;
        if !base.lower_scaled.is_positive() || !alpha.lower_scaled.is_positive() {
            return Err(FixedPointError::NonpositiveInput);
        }
        let logarithm = self.logarithm(base)?;
        let scaled_logarithm = self.multiply(alpha, &logarithm)?;
        let exponent = self.negate(&scaled_logarithm)?;
        self.exponential(&exponent)
    }

    /// Adds two intervals with the configured word checks.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched precision or a word-bound breach.
    pub fn add_intervals(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.add(left, right)
    }

    /// Subtracts two intervals with outward rounding.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched precision or a word-bound breach.
    pub fn subtract_intervals(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.subtract(left, right)
    }

    /// Multiplies two intervals with outward rounding.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched precision or a word-bound breach.
    pub fn multiply_intervals(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.multiply(left, right)
    }

    /// Divides by a wholly positive interval with outward rounding.
    ///
    /// # Errors
    ///
    /// Returns an error unless the denominator is wholly positive, or for a
    /// precision/word-bound violation.
    pub fn divide_intervals(
        &mut self,
        numerator: &DyadicInterval,
        denominator: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.divide(numerator, denominator)
    }

    /// Multiplies an interval by an exact integer.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched precision or a word-bound breach.
    pub fn multiply_interval_integer(
        &mut self,
        value: &DyadicInterval,
        factor: i128,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.multiply_integer(value, factor)
    }

    fn logarithm_point(&mut self, scaled: &BigInt) -> Result<DyadicInterval, FixedPointError> {
        let floor_log = scaled
            .magnitude()
            .bits()
            .checked_sub(1)
            .ok_or(FixedPointError::NonpositiveInput)?;
        let exponent = i128::from(floor_log) - i128::from(self.config.fractional_bits);
        let point = self.interval(scaled.clone(), scaled.clone())?;
        let reduced = self.shift_power_two(&point, -exponent)?;
        let one = self.integer_interval(1)?;
        let numerator = self.subtract(&reduced, &one)?;
        let denominator = self.add(&reduced, &one)?;
        let transformed = self.divide(&numerator, &denominator)?;
        let reduced_log = self.atanh_log_series(&transformed)?;

        let third = self.ratio_interval(BigInt::one(), &BigInt::from(3_u8))?;
        let log_two = self.atanh_log_series(&third)?;
        let exponent_log_two = self.multiply_integer(&log_two, exponent)?;
        self.add(&reduced_log, &exponent_log_two)
    }

    fn atanh_log_series(
        &mut self,
        transformed: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        let magnitude = self.magnitude(transformed)?;
        if magnitude.upper_scaled >= self.scale {
            return Err(FixedPointError::InsufficientPrecision);
        }
        let squared = self.multiply(transformed, transformed)?;
        let mut power = transformed.clone();
        let mut sum = self.integer_interval(0)?;
        for term_index in 0..self.config.series_terms {
            let denominator = u64::from(term_index) * 2 + 1;
            let term = self.divide_integer(&power, denominator)?;
            sum = self.add(&sum, &term)?;
            power = self.multiply(&power, &squared)?;
            self.metrics.logarithm_series_terms = self
                .metrics
                .logarithm_series_terms
                .checked_add(1)
                .ok_or(FixedPointError::MetricOverflow)?;
        }

        let power_magnitude = self.magnitude(&power)?;
        let first_denominator = u64::from(self.config.series_terms) * 2 + 1;
        let tail_start = self.divide_integer(&power_magnitude, first_denominator)?;
        let squared_magnitude = self.multiply(&magnitude, &magnitude)?;
        let one = self.integer_interval(1)?;
        let tail_denominator = self.subtract(&one, &squared_magnitude)?;
        let tail = self.divide(&tail_start, &tail_denominator)?;
        let doubled_sum = self.multiply_integer(&sum, 2)?;
        let doubled_tail = self.multiply_integer(&tail, 2)?;
        self.add_symmetric_error(&doubled_sum, &doubled_tail)
    }

    fn exponential_point(&mut self, scaled: &BigInt) -> Result<DyadicInterval, FixedPointError> {
        let point = self.interval(scaled.clone(), scaled.clone())?;
        let mut reductions = 0_u32;
        let mut reduced = point;
        while self.magnitude(&reduced)?.upper_scaled * 8 > self.scale {
            reduced = self.shift_power_two(&reduced, -1)?;
            reductions = reductions
                .checked_add(1)
                .ok_or(FixedPointError::InsufficientPrecision)?;
            if u64::from(reductions) > self.config.maximum_word_bits {
                return Err(FixedPointError::InsufficientPrecision);
            }
        }

        let one = self.integer_interval(1)?;
        let mut sum = one.clone();
        let mut term = one.clone();
        for order in 1..=self.config.series_terms {
            term = self.multiply(&term, &reduced)?;
            term = self.divide_integer(&term, u64::from(order))?;
            sum = self.add(&sum, &term)?;
            self.metrics.exponential_series_terms = self
                .metrics
                .exponential_series_terms
                .checked_add(1)
                .ok_or(FixedPointError::MetricOverflow)?;
        }

        let next_numerator = self.multiply(&term, &reduced)?;
        let next_term =
            self.divide_integer(&next_numerator, u64::from(self.config.series_terms) + 1)?;
        let magnitude = self.magnitude(&reduced)?;
        let tail_ratio =
            self.divide_integer(&magnitude, u64::from(self.config.series_terms) + 2)?;
        let tail_denominator = self.subtract(&one, &tail_ratio)?;
        let next_magnitude = self.magnitude(&next_term)?;
        let tail = self.divide(&next_magnitude, &tail_denominator)?;
        let mut result = self.add_symmetric_error(&sum, &tail)?;
        if !result.lower_scaled.is_positive() {
            return Err(FixedPointError::InsufficientPrecision);
        }
        for _ in 0..reductions {
            result = self.multiply(&result, &result)?;
        }
        Ok(result)
    }

    fn ratio_interval(
        &mut self,
        numerator: BigInt,
        denominator: &BigInt,
    ) -> Result<DyadicInterval, FixedPointError> {
        if !denominator.is_positive() {
            return Err(FixedPointError::InvalidRatio);
        }
        let scaled = numerator * &self.scale;
        self.observe(&scaled)?;
        let lower = scaled.div_floor(denominator);
        let upper = scaled.div_ceil(denominator);
        self.metrics.outward_rounds = self
            .metrics
            .outward_rounds
            .checked_add(2)
            .ok_or(FixedPointError::MetricOverflow)?;
        self.interval(lower, upper)
    }

    fn integer_interval(&mut self, value: i128) -> Result<DyadicInterval, FixedPointError> {
        let scaled = BigInt::from(value) * &self.scale;
        self.interval(scaled.clone(), scaled)
    }

    fn interval(
        &mut self,
        lower_scaled: BigInt,
        upper_scaled: BigInt,
    ) -> Result<DyadicInterval, FixedPointError> {
        if lower_scaled > upper_scaled {
            return Err(FixedPointError::InvalidInterval);
        }
        self.observe(&lower_scaled)?;
        self.observe(&upper_scaled)?;
        Ok(DyadicInterval {
            lower_scaled,
            upper_scaled,
            fractional_bits: self.config.fractional_bits,
        })
    }

    fn validate_interval(&self, interval: &DyadicInterval) -> Result<(), FixedPointError> {
        if interval.fractional_bits != self.config.fractional_bits
            || interval.lower_scaled > interval.upper_scaled
        {
            return Err(FixedPointError::InvalidInterval);
        }
        Ok(())
    }

    fn add(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(left)?;
        self.validate_interval(right)?;
        self.operation()?;
        self.interval(
            &left.lower_scaled + &right.lower_scaled,
            &left.upper_scaled + &right.upper_scaled,
        )
    }

    fn subtract(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(left)?;
        self.validate_interval(right)?;
        self.operation()?;
        self.interval(
            &left.lower_scaled - &right.upper_scaled,
            &left.upper_scaled - &right.lower_scaled,
        )
    }

    fn negate(&mut self, value: &DyadicInterval) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        self.operation()?;
        self.interval(-&value.upper_scaled, -&value.lower_scaled)
    }

    fn multiply(
        &mut self,
        left: &DyadicInterval,
        right: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(left)?;
        self.validate_interval(right)?;
        let products = [
            &left.lower_scaled * &right.lower_scaled,
            &left.lower_scaled * &right.upper_scaled,
            &left.upper_scaled * &right.lower_scaled,
            &left.upper_scaled * &right.upper_scaled,
        ];
        for product in &products {
            self.observe(product)?;
        }
        let lower_product = products
            .iter()
            .min()
            .ok_or(FixedPointError::InvalidInterval)?;
        let upper_product = products
            .iter()
            .max()
            .ok_or(FixedPointError::InvalidInterval)?;
        let lower = lower_product.div_floor(&self.scale);
        let upper = upper_product.div_ceil(&self.scale);
        self.rounded_operation(2)?;
        self.interval(lower, upper)
    }

    fn multiply_integer(
        &mut self,
        value: &DyadicInterval,
        factor: i128,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        self.operation()?;
        let factor = BigInt::from(factor);
        let first = &value.lower_scaled * &factor;
        let second = &value.upper_scaled * factor;
        self.interval(first.clone().min(second.clone()), first.max(second))
    }

    fn divide_integer(
        &mut self,
        value: &DyadicInterval,
        divisor: u64,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        if divisor == 0 {
            return Err(FixedPointError::InvalidRatio);
        }
        let divisor = BigInt::from(divisor);
        let lower = value.lower_scaled.div_floor(&divisor);
        let upper = value.upper_scaled.div_ceil(&divisor);
        self.rounded_operation(2)?;
        self.interval(lower, upper)
    }

    fn reciprocal(&mut self, value: &DyadicInterval) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        if !value.lower_scaled.is_positive() {
            return Err(FixedPointError::NonpositiveInput);
        }
        let numerator = &self.scale * &self.scale;
        self.observe(&numerator)?;
        let lower = numerator.div_floor(&value.upper_scaled);
        let upper = numerator.div_ceil(&value.lower_scaled);
        self.rounded_operation(2)?;
        self.interval(lower, upper)
    }

    fn divide(
        &mut self,
        numerator: &DyadicInterval,
        denominator: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        let reciprocal = self.reciprocal(denominator)?;
        self.multiply(numerator, &reciprocal)
    }

    fn shift_power_two(
        &mut self,
        value: &DyadicInterval,
        exponent: i128,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        if exponent >= 0 {
            let exponent =
                usize::try_from(exponent).map_err(|_| FixedPointError::WordSizeExceeded {
                    observed: u64::MAX,
                    limit: self.config.maximum_word_bits,
                })?;
            let factor = BigInt::one() << exponent;
            self.operation()?;
            return self.interval(&value.lower_scaled * &factor, &value.upper_scaled * factor);
        }
        let magnitude = exponent
            .checked_neg()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(FixedPointError::InvalidInterval)?;
        let divisor = BigInt::one() << magnitude;
        let lower = value.lower_scaled.div_floor(&divisor);
        let upper = value.upper_scaled.div_ceil(&divisor);
        self.rounded_operation(2)?;
        self.interval(lower, upper)
    }

    fn magnitude(&mut self, value: &DyadicInterval) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        let upper = value.lower_scaled.abs().max(value.upper_scaled.abs());
        self.interval(BigInt::zero(), upper)
    }

    fn add_symmetric_error(
        &mut self,
        value: &DyadicInterval,
        error: &DyadicInterval,
    ) -> Result<DyadicInterval, FixedPointError> {
        self.validate_interval(value)?;
        self.validate_interval(error)?;
        if error.lower_scaled.is_negative() {
            return Err(FixedPointError::InvalidInterval);
        }
        self.operation()?;
        self.interval(
            &value.lower_scaled - &error.upper_scaled,
            &value.upper_scaled + &error.upper_scaled,
        )
    }

    fn operation(&mut self) -> Result<(), FixedPointError> {
        self.metrics.arithmetic_operations = self
            .metrics
            .arithmetic_operations
            .checked_add(1)
            .ok_or(FixedPointError::MetricOverflow)?;
        Ok(())
    }

    fn rounded_operation(&mut self, rounds: u64) -> Result<(), FixedPointError> {
        self.operation()?;
        self.metrics.outward_rounds = self
            .metrics
            .outward_rounds
            .checked_add(rounds)
            .ok_or(FixedPointError::MetricOverflow)?;
        Ok(())
    }

    fn observe(&mut self, value: &BigInt) -> Result<(), FixedPointError> {
        let bits = value.magnitude().bits();
        self.metrics.maximum_observed_word_bits = self.metrics.maximum_observed_word_bits.max(bits);
        if bits > self.config.maximum_word_bits {
            return Err(FixedPointError::WordSizeExceeded {
                observed: bits,
                limit: self.config.maximum_word_bits,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FixedPointError {
    #[error("the fixed-point configuration is invalid")]
    InvalidConfig,
    #[error("the ratio must have a positive denominator")]
    InvalidRatio,
    #[error("the dyadic interval is malformed or uses a different precision")]
    InvalidInterval,
    #[error("the operation requires a wholly positive interval")]
    NonpositiveInput,
    #[error("fixed-point word size {observed} exceeds the checked limit {limit}")]
    WordSizeExceeded { observed: u64, limit: u64 },
    #[error("the selected precision or series length cannot certify the result")]
    InsufficientPrecision,
    #[error("an arithmetic-work counter overflowed")]
    MetricOverflow,
}

fn ceil_log2(value: u64) -> u64 {
    if value <= 1 {
        0
    } else {
        u64::from(u64::BITS - (value - 1).leading_zeros())
    }
}

#[cfg(test)]
mod tests {
    use super::{CertifiedFixedPoint, FixedPointConfig, FixedPointError};

    fn engine() -> CertifiedFixedPoint {
        CertifiedFixedPoint::new(FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap())
            .unwrap()
    }

    #[test]
    fn certifies_logarithm_and_exponential_enclosures() {
        let mut arithmetic = engine();
        let two = arithmetic.enclose_ratio(2, 1).unwrap();
        let log_two = arithmetic.logarithm(&two).unwrap();
        assert!(
            log_two
                .overlaps_ratio_interval(
                    693_147_180_559_945_309,
                    693_147_180_559_945_310,
                    1_000_000_000_000_000_000,
                )
                .unwrap()
        );
        assert!(log_two.width_at_most(1, 1_000_000_000_000_000).unwrap());

        let recovered = arithmetic.exponential(&log_two).unwrap();
        assert!(recovered.contains_ratio(2, 1).unwrap());
        assert!(recovered.width_at_most(1, 1_000_000_000_000).unwrap());
        assert!(arithmetic.metrics().outward_rounds > 0);
        assert!(
            arithmetic.metrics().maximum_observed_word_bits
                <= arithmetic.config().maximum_word_bits
        );
    }

    #[test]
    fn certifies_fractional_negative_power() {
        let mut arithmetic = engine();
        let four = arithmetic.enclose_ratio(4, 1).unwrap();
        let half = arithmetic.enclose_ratio(1, 2).unwrap();
        let inverse_square_root = arithmetic.negative_power(&four, &half).unwrap();
        assert!(inverse_square_root.contains_ratio(1, 2).unwrap());
        assert!(
            inverse_square_root
                .width_at_most(1, 1_000_000_000_000)
                .unwrap()
        );
    }

    #[test]
    fn encloses_exact_identities_across_power_of_two_ranges() {
        let mut arithmetic = engine();
        for (numerator, denominator) in [(1, 8), (1, 2), (1, 1), (2, 1), (8, 1)] {
            let value = arithmetic.enclose_ratio(numerator, denominator).unwrap();
            let logarithm = arithmetic.logarithm(&value).unwrap();
            let recovered = arithmetic.exponential(&logarithm).unwrap();
            assert!(
                recovered.contains_ratio(numerator, denominator).unwrap(),
                "failed exp(log(x)) enclosure for {numerator}/{denominator}"
            );

            let one = arithmetic.enclose_ratio(1, 1).unwrap();
            let reciprocal = arithmetic.negative_power(&value, &one).unwrap();
            assert!(
                reciprocal.contains_ratio(denominator, numerator).unwrap(),
                "failed reciprocal enclosure for {numerator}/{denominator}"
            );
        }
    }

    #[test]
    fn rejects_nonpositive_transcendental_domain() {
        let mut arithmetic = engine();
        let zero = arithmetic.enclose_ratio(0, 1).unwrap();
        assert_eq!(
            arithmetic.logarithm(&zero),
            Err(FixedPointError::NonpositiveInput)
        );
        let one = arithmetic.enclose_ratio(1, 1).unwrap();
        assert_eq!(
            arithmetic.negative_power(&one, &zero),
            Err(FixedPointError::NonpositiveInput)
        );
    }

    #[test]
    fn enforces_polylogarithmic_word_budget() {
        assert_eq!(
            FixedPointConfig::source_bounded(2, 32, 16, 1),
            Err(FixedPointError::InvalidConfig)
        );
        let config = FixedPointConfig::source_bounded(64, 16, 16, 2).unwrap();
        let mut arithmetic = CertifiedFixedPoint::new(config).unwrap();
        let huge = arithmetic.enclose_ratio(i128::MAX, 1);
        assert!(matches!(
            huge,
            Err(FixedPointError::WordSizeExceeded { .. })
        ));
    }
}
