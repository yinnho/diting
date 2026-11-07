//! Helper trait to calculate dimensions during layout resolution

use crate::geometry::{Rect, Size};
use crate::style::{Dimension, LengthPercentage, LengthPercentageAuto};
use crate::style_helpers::TaffyZero;
use crate::CompactLength;

/// Trait to encapsulate behaviour where we need to resolve from a
/// potentially context-dependent size or dimension into
/// a context-independent size or dimension.
///
/// Will return a `None` if it unable to resolve.
pub trait MaybeResolve<In, Out> {
    /// Resolve a dimension that might be dependent on a context, with `None` as fallback value
    fn maybe_resolve(self, context: In, calc: impl Fn(*const (), f32) -> f32) -> Out;
}

/// Trait to encapsulate behaviour where we need to resolve from a
/// potentially context-dependent size or dimension into
/// a context-independent size or dimension.
///
/// Will return a default value if it unable to resolve.
pub trait ResolveOrZero<TContext, TOutput: TaffyZero> {
    /// Resolve a dimension that might be dependent on a context, with a default fallback value
    fn resolve_or_zero(self, context: TContext, calc: impl Fn(*const (), f32) -> f32) -> TOutput;
}

impl MaybeResolve<Option<f32>, Option<f32>> for LengthPercentage {
    /// Converts the given [`LengthPercentage`] into an absolute length
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> Option<f32> {
        match self.0.tag() {
            CompactLength::LENGTH_TAG => Some(self.0.value()),
            CompactLength::PERCENT_TAG => context.map(|dim| dim * self.0.value()),
            #[cfg(feature = "calc")]
            _ if self.0.is_plain_calc() => context.map(|dim| calc(self.0.calc_value(), dim)),
            _ => unreachable!(),
        }
    }
}

impl MaybeResolve<Option<f32>, Option<f32>> for LengthPercentageAuto {
    /// Converts the given [`LengthPercentageAuto`] into an absolute length
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> Option<f32> {
        match self.0.tag() {
            CompactLength::AUTO_TAG => None,
            CompactLength::LENGTH_TAG => Some(self.0.value()),
            CompactLength::PERCENT_TAG => context.map(|dim| dim * self.0.value()),
            #[cfg(feature = "calc")]
            _ if self.0.is_plain_calc() => context.map(|dim| calc(self.0.calc_value(), dim)),
            _ => unreachable!(),
        }
    }
}

impl MaybeResolve<Option<f32>, Option<f32>> for Dimension {
    /// Converts the given [`Dimension`] into an absolute length
    ///
    /// Can return `None`
    fn maybe_resolve(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> Option<f32> {
        match self.0.tag() {
            CompactLength::AUTO_TAG => None,
            // The content keyword is only valid for flex-basis. In any other context it behaves as auto.
            CompactLength::CONTENT_TAG => None,
            CompactLength::LENGTH_TAG => Some(self.0.value()),
            CompactLength::PERCENT_TAG => context.map(|dim| dim * self.0.value()),
            #[cfg(feature = "calc")]
            _ if self.0.is_plain_calc() => context.map(|dim| calc(self.0.calc_value(), dim)),
            // Intrinsic sizing keywords cannot be resolved to a definite size out of context.
            // Layout algorithms that support them must handle them explicitly.
            _ if self.0.is_sizing_keyword() => None,
            _ => unreachable!(),
        }
    }
}

// Generic implementation of MaybeResolve for f32 context where MaybeResolve is implemented
// for Option<f32> context
impl<T: MaybeResolve<Option<f32>, Option<f32>>> MaybeResolve<f32, Option<f32>> for T {
    /// Converts the given MaybeResolve value into an absolute length
    /// Can return `None`
    fn maybe_resolve(self, context: f32, calc: impl Fn(*const (), f32) -> f32) -> Option<f32> {
        self.maybe_resolve(Some(context), calc)
    }
}

// Generic MaybeResolve for Size
impl<In, Out, T: MaybeResolve<In, Out>> MaybeResolve<Size<In>, Size<Out>> for Size<T> {
    /// Converts any `parent`-relative values for size into an absolute size
    #[inline(always)]
    fn maybe_resolve(self, context: Size<In>, calc: impl Fn(*const (), f32) -> f32) -> Size<Out> {
        Size {
            width: self.width.maybe_resolve(context.width, &calc),
            height: self.height.maybe_resolve(context.height, &calc),
        }
    }
}

impl ResolveOrZero<Option<f32>, f32> for LengthPercentage {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> f32 {
        self.maybe_resolve(context, calc).unwrap_or(0.0)
    }
}

impl ResolveOrZero<Option<f32>, f32> for LengthPercentageAuto {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> f32 {
        self.maybe_resolve(context, calc).unwrap_or(0.0)
    }
}

impl ResolveOrZero<Option<f32>, f32> for Dimension {
    /// Will return a default value of result is evaluated to `None`
    fn resolve_or_zero(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> f32 {
        self.maybe_resolve(context, calc).unwrap_or(0.0)
    }
}

// Generic ResolveOrZero for Size
impl<In, Out: TaffyZero, T: ResolveOrZero<In, Out>> ResolveOrZero<Size<In>, Size<Out>> for Size<T> {
    /// Converts any `parent`-relative values for size into an absolute size
    fn resolve_or_zero(self, context: Size<In>, calc: impl Fn(*const (), f32) -> f32) -> Size<Out> {
        Size {
            width: self.width.resolve_or_zero(context.width, &calc),
            height: self.height.resolve_or_zero(context.height, &calc),
        }
    }
}

// Generic ResolveOrZero for resolving Rect against Size
impl<In: Copy, Out: TaffyZero, T: ResolveOrZero<In, Out>> ResolveOrZero<Size<In>, Rect<Out>> for Rect<T> {
    /// Converts any `parent`-relative values for Rect into an absolute Rect
    fn resolve_or_zero(self, context: Size<In>, calc: impl Fn(*const (), f32) -> f32) -> Rect<Out> {
        Rect {
            left: self.left.resolve_or_zero(context.width, &calc),
            right: self.right.resolve_or_zero(context.width, &calc),
            top: self.top.resolve_or_zero(context.height, &calc),
            bottom: self.bottom.resolve_or_zero(context.height, &calc),
        }
    }
}

// Generic ResolveOrZero for resolving Rect against Option
impl<Out: TaffyZero, T: ResolveOrZero<Option<f32>, Out>> ResolveOrZero<Option<f32>, Rect<Out>> for Rect<T> {
    /// Converts any `parent`-relative values for Rect into an absolute Rect
    #[inline(always)]
    fn resolve_or_zero(self, context: Option<f32>, calc: impl Fn(*const (), f32) -> f32) -> Rect<Out> {
        Rect {
            left: self.left.resolve_or_zero(context, &calc),
            right: self.right.resolve_or_zero(context, &calc),
            top: self.top.resolve_or_zero(context, &calc),
            bottom: self.bottom.resolve_or_zero(context, &calc),
        }
    }
}

// Generic ResolveOrZero for resolving Rect against f32
impl<Out: TaffyZero, T: ResolveOrZero<f32, Out>> ResolveOrZero<f32, Rect<Out>> for Rect<T> {
    /// Converts any `parent`-relative values for Rect into an absolute Rect
    fn resolve_or_zero(self, context: f32, calc: impl Fn(*const (), f32) -> f32) -> Rect<Out> {
        Rect {
            left: self.left.resolve_or_zero(context, &calc),
            right: self.right.resolve_or_zero(context, &calc),
            top: self.top.resolve_or_zero(context, &calc),
            bottom: self.bottom.resolve_or_zero(context, &calc),
        }
    }
}

