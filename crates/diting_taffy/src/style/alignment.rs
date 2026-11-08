//! Style types for controlling alignment.
//!
//! The public alignment types ([`AlignItems`], [`AlignContent`], and their aliases) are
//! structs with two orthogonal fields: a *position* keyword
//! ([`AlignItemsKeyword`] / [`AlignContentKeyword`]) and an *overflow-position*
//! modifier ([`AlignmentSafety`]). The pre-existing CSS spellings — `Start`, `End`,
//! `FlexStart`, `FlexEnd`, `Center`, `Stretch`, `SpaceBetween`, …, `SafeStart`,
//! `SafeEnd`, `SafeFlexStart`, `SafeFlexEnd`, `SafeCenter` — are exposed as associated
//! constants on the structs, so call sites read identically to the previous enum form.


use crate::style::Direction;

/// The position-keyword half of [`AlignItems`] (and its aliases `AlignSelf`,
/// `JustifyItems`, `JustifySelf`).
///
/// Compute paths match on this enum directly so every match is exhaustive and
/// requires no `Safe*` siblings.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum AlignItemsKeyword {
    /// The layout mode's default alignment: usually `Stretch`, but `Start` in some cases.
    #[default]
    Normal,
    /// Items are packed toward the start of the axis.
    Start,
    /// Items are packed toward the end of the axis.
    End,
    /// Items are packed towards the flex-relative start of the axis.
    ///
    /// For flex containers with flex_direction RowReverse or ColumnReverse this is
    /// equivalent to End. In all other cases it is equivalent to Start.
    FlexStart,
    /// Items are packed towards the flex-relative end of the axis.
    ///
    /// For flex containers with flex_direction RowReverse or ColumnReverse this is
    /// equivalent to Start. In all other cases it is equivalent to End.
    FlexEnd,
    /// Items are packed toward the start of the axis as determined by the item's own
    /// writing mode/direction (rather than the container's).
    ///
    /// Equivalent to Start when the item's `direction` matches the container's, and
    /// to End when they differ (in the inline axis).
    SelfStart,
    /// Items are packed toward the end of the axis as determined by the item's own
    /// writing mode/direction (rather than the container's).
    ///
    /// Equivalent to End when the item's `direction` matches the container's, and
    /// to Start when they differ (in the inline axis).
    SelfEnd,
    /// Items are packed along the center of the cross axis.
    Center,
    /// Items are aligned such as their baselines align.
    Baseline,
    /// Stretch to fill the container.
    Stretch,
}

/// The position-keyword half of [`AlignContent`] (and its alias `JustifyContent`).
///
/// Compute paths match on this enum directly so every match is exhaustive and
/// requires no `Safe*` siblings.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum AlignContentKeyword {
    /// The layout mode's default alignment: `Stretch` for flex and grid containers, `Start`
    /// for block containers.
    #[default]
    Normal,
    /// Items are packed toward the start of the axis.
    Start,
    /// Items are packed toward the end of the axis.
    End,
    /// Items are packed towards the flex-relative start of the axis.
    FlexStart,
    /// Items are packed towards the flex-relative end of the axis.
    FlexEnd,
    /// Items are centered around the middle of the axis.
    Center,
    /// Items are stretched to fill the container.
    Stretch,
    /// The first and last items are aligned flush with the edges of the container
    /// (no gap). The gap between items is distributed evenly.
    SpaceBetween,
    /// The gap between the first and last items is exactly THE SAME as the gap
    /// between items. The gaps are distributed evenly.
    SpaceEvenly,
    /// The gap between the first and last items is exactly HALF the gap between
    /// items. The gaps are distributed evenly in proportion to these ratios.
    SpaceAround,
}

impl AlignContentKeyword {
    /// Returns the reversed keyword for RTL (right-to-left) contexts: `Start`↔`End`,
    /// `FlexStart`↔`FlexEnd`. `Stretch` maps to `End` to preserve the layout
    /// algorithms' historical handling. `Normal`, `Center` and the distribution keywords
    /// (`SpaceBetween`, `SpaceEvenly`, `SpaceAround`) are unaffected because their
    /// visual placement is direction-symmetric.
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
            Self::FlexStart => Self::FlexEnd,
            Self::FlexEnd => Self::FlexStart,
            Self::Stretch => Self::End,
            Self::Normal | Self::Center | Self::SpaceBetween | Self::SpaceEvenly | Self::SpaceAround => self,
        }
    }
}

/// The overflow-position modifier per [CSS Box Alignment §4.3][css-align-overflow].
///
/// `Safe` falls back to start-edge alignment when the alignment subject would
/// overflow the alignment container, so the start of the content stays visible.
/// `Unsafe` keeps the requested alignment even when that causes overflow at the
/// start edge. `Default` (no overflow-position keyword specified) behaves as
/// `Unsafe`, except for absolutely positioned boxes whose alignment is instead
/// adjusted to minimize overflow per [CSS Box Alignment §4.4.1.2][css-align-abspos].
///
/// CSS only defines `safe` / `unsafe` against the position values `start`, `end`,
/// `flex-start`, `flex-end`, `center`. The struct shape does not enforce that
/// constraint at the type level — the parser rejects invalid combinations, and
/// the compute pass treats `Safe` paired with a non-position keyword (`Stretch`,
/// `Baseline`, `Space*`) the same as `Unsafe`.
///
/// [css-align-overflow]: https://www.w3.org/TR/css-align-3/#overflow-values
/// [css-align-abspos]: https://www.w3.org/TR/css-align-3/#auto-safety-position
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum AlignmentSafety {
    /// No overflow-position keyword specified. Behaves as `Unsafe`, except for absolutely
    /// positioned boxes, whose alignment is adjusted to minimize overflow of their
    /// containing block.
    #[default]
    Default,
    /// Keeps the requested alignment even when the subject overflows the alignment
    /// container at the start edge.
    Unsafe,
    /// Falls back to the start edge when the subject would overflow, to avoid data
    /// loss.
    Safe,
}

/// Used to control how child nodes are aligned.
/// For Flexbox it controls alignment in the cross axis.
/// For Grid it controls alignment in the block axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-items)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct AlignItems {
    /// Position keyword.
    pub keyword: AlignItemsKeyword,
    /// Overflow-position modifier (`safe` / `unsafe`).
    pub safety: AlignmentSafety,
}

impl AlignItems {
    /// The layout mode's default alignment. See [`AlignItemsKeyword::Normal`].
    pub const NORMAL: Self = Self { keyword: AlignItemsKeyword::Normal, safety: AlignmentSafety::Default };
    /// Items are packed toward the start of the axis.
    pub const START: Self = Self { keyword: AlignItemsKeyword::Start, safety: AlignmentSafety::Default };
    /// Items are packed toward the end of the axis.
    pub const END: Self = Self { keyword: AlignItemsKeyword::End, safety: AlignmentSafety::Default };
    /// Items are packed towards the flex-relative start of the axis.
    pub const FLEX_START: Self = Self { keyword: AlignItemsKeyword::FlexStart, safety: AlignmentSafety::Default };
    /// Items are packed towards the flex-relative end of the axis.
    pub const FLEX_END: Self = Self { keyword: AlignItemsKeyword::FlexEnd, safety: AlignmentSafety::Default };
    /// Items are packed toward the start of the axis as determined by the item's own direction.
    pub const SELF_START: Self = Self { keyword: AlignItemsKeyword::SelfStart, safety: AlignmentSafety::Default };
    /// Items are packed toward the end of the axis as determined by the item's own direction.
    pub const SELF_END: Self = Self { keyword: AlignItemsKeyword::SelfEnd, safety: AlignmentSafety::Default };
    /// Items are packed along the center of the cross axis.
    pub const CENTER: Self = Self { keyword: AlignItemsKeyword::Center, safety: AlignmentSafety::Default };
    /// Items are aligned such as their baselines align.
    pub const BASELINE: Self = Self { keyword: AlignItemsKeyword::Baseline, safety: AlignmentSafety::Default };
    /// Stretch to fill the container.
    pub const STRETCH: Self = Self { keyword: AlignItemsKeyword::Stretch, safety: AlignmentSafety::Default };
    /// Like [`AlignItems::START`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_START: Self = Self { keyword: AlignItemsKeyword::Start, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::END`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_END: Self = Self { keyword: AlignItemsKeyword::End, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::FLEX_START`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_FLEX_START: Self = Self { keyword: AlignItemsKeyword::FlexStart, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::FLEX_END`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_FLEX_END: Self = Self { keyword: AlignItemsKeyword::FlexEnd, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::CENTER`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_CENTER: Self = Self { keyword: AlignItemsKeyword::Center, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::SELF_START`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_SELF_START: Self = Self { keyword: AlignItemsKeyword::SelfStart, safety: AlignmentSafety::Safe };
    /// Like [`AlignItems::SELF_END`], but falls back to [`AlignItems::START`] when the
    /// alignment subject overflows the alignment container, to avoid data loss.
    pub const SAFE_SELF_END: Self = Self { keyword: AlignItemsKeyword::SelfEnd, safety: AlignmentSafety::Safe };

    /// Returns `true` iff this carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(self.safety, AlignmentSafety::Safe)
    }

    /// Returns this alignment with the given overflow-position modifier.
    #[inline]
    pub const fn with_safety(self, safety: AlignmentSafety) -> Self {
        Self { keyword: self.keyword, safety }
    }

    /// Returns the underlying position keyword, discarding the safety modifier.
    #[inline]
    pub const fn keyword(self) -> AlignItemsKeyword {
        self.keyword
    }

    /// Returns true if the keyword is `Stretch` or `Normal` (ignoring any overflow-position
    /// modifier).
    #[inline(always)]
    pub const fn is_stretch_or_normal(self) -> bool {
        matches!(self.keyword, AlignItemsKeyword::Stretch | AlignItemsKeyword::Normal)
    }

    /// Resolve the writing-mode-relative `SelfStart`/`SelfEnd` keywords to `Start`/`End`
    /// based on the item's own `direction` per CSS Box Alignment §5.2
    /// <https://www.w3.org/TR/css-align-3/#self-alignment>. All other keywords are
    /// returned unchanged.
    ///
    /// The `Start`/`End` keywords used by the compute paths are relative to the
    /// *container's* writing mode/direction, so in the inline axis `SelfStart` resolves
    /// to `Start` when the item's direction matches the container's and to `End` when it
    /// differs. Taffy only supports the `horizontal-tb` writing mode, so in the block
    /// axis `SelfStart`/`SelfEnd` always resolve to `Start`/`End` respectively.
    #[inline]
    pub fn resolve_self_relative(
        self,
        item_direction: Direction,
        container_direction: Direction,
        axis_is_inline: bool,
    ) -> Self {
        let flip = axis_is_inline && item_direction != container_direction;
        let keyword = match self.keyword {
            AlignItemsKeyword::SelfStart => {
                if flip {
                    AlignItemsKeyword::End
                } else {
                    AlignItemsKeyword::Start
                }
            }
            AlignItemsKeyword::SelfEnd => {
                if flip {
                    AlignItemsKeyword::Start
                } else {
                    AlignItemsKeyword::End
                }
            }
            other => other,
        };
        Self { keyword, safety: self.safety }
    }
}



/// Used to control how child nodes are aligned.
/// Does not apply to Flexbox, and will be ignored if specified on a flex container.
/// For Grid it controls alignment in the inline axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-items)
pub type JustifyItems = AlignItems;
/// Controls alignment of an individual node.
///
/// Overrides the parent Node's `AlignItems` property.
/// For Flexbox it controls alignment in the cross axis.
/// For Grid it controls alignment in the block axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-self)
pub type AlignSelf = AlignItems;
/// Controls alignment of an individual node.
///
/// Overrides the parent Node's `JustifyItems` property.
/// Does not apply to Flexbox, and will be ignored if specified on a flex child.
/// For Grid it controls alignment in the inline axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-self)
pub type JustifySelf = AlignItems;

/// Sets the distribution of space between and around content items.
/// For Flexbox it controls alignment in the cross axis.
/// For Grid it controls alignment in the block axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/align-content)
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct AlignContent {
    /// Position keyword.
    pub keyword: AlignContentKeyword,
    /// Overflow-position modifier (`safe` / `unsafe`).
    pub safety: AlignmentSafety,
}

impl AlignContent {
    /// The layout mode's default alignment. See [`AlignContentKeyword::Normal`].
    pub const NORMAL: Self = Self { keyword: AlignContentKeyword::Normal, safety: AlignmentSafety::Default };
    /// Items are packed toward the start of the axis.
    pub const START: Self = Self { keyword: AlignContentKeyword::Start, safety: AlignmentSafety::Default };
    /// Items are packed toward the end of the axis.
    pub const END: Self = Self { keyword: AlignContentKeyword::End, safety: AlignmentSafety::Default };
    /// Items are packed towards the flex-relative start of the axis.
    pub const FLEX_START: Self = Self { keyword: AlignContentKeyword::FlexStart, safety: AlignmentSafety::Default };
    /// Items are packed towards the flex-relative end of the axis.
    pub const FLEX_END: Self = Self { keyword: AlignContentKeyword::FlexEnd, safety: AlignmentSafety::Default };
    /// Items are centered around the middle of the axis.
    pub const CENTER: Self = Self { keyword: AlignContentKeyword::Center, safety: AlignmentSafety::Default };
    /// Items are stretched to fill the container.
    pub const STRETCH: Self = Self { keyword: AlignContentKeyword::Stretch, safety: AlignmentSafety::Default };
    /// The first and last items are aligned flush with the edges of the container.
    pub const SPACE_BETWEEN: Self =
        Self { keyword: AlignContentKeyword::SpaceBetween, safety: AlignmentSafety::Default };
    /// The gap between the first and last items equals the gap between items.
    pub const SPACE_EVENLY: Self = Self { keyword: AlignContentKeyword::SpaceEvenly, safety: AlignmentSafety::Default };
    /// The gap between the first and last items is half the gap between items.
    pub const SPACE_AROUND: Self = Self { keyword: AlignContentKeyword::SpaceAround, safety: AlignmentSafety::Default };
    /// Like [`AlignContent::START`], but falls back to [`AlignContent::START`] when the
    /// content overflows the alignment container, to avoid data loss.
    pub const SAFE_START: Self = Self { keyword: AlignContentKeyword::Start, safety: AlignmentSafety::Safe };
    /// Like [`AlignContent::END`], but falls back to [`AlignContent::START`] when the
    /// content overflows the alignment container, to avoid data loss.
    pub const SAFE_END: Self = Self { keyword: AlignContentKeyword::End, safety: AlignmentSafety::Safe };
    /// Like [`AlignContent::FLEX_START`], but falls back to [`AlignContent::START`] when
    /// the content overflows the alignment container, to avoid data loss.
    pub const SAFE_FLEX_START: Self = Self { keyword: AlignContentKeyword::FlexStart, safety: AlignmentSafety::Safe };
    /// Like [`AlignContent::FLEX_END`], but falls back to [`AlignContent::START`] when the
    /// content overflows the alignment container, to avoid data loss.
    pub const SAFE_FLEX_END: Self = Self { keyword: AlignContentKeyword::FlexEnd, safety: AlignmentSafety::Safe };
    /// Like [`AlignContent::CENTER`], but falls back to [`AlignContent::START`] when the
    /// content overflows the alignment container, to avoid data loss.
    pub const SAFE_CENTER: Self = Self { keyword: AlignContentKeyword::Center, safety: AlignmentSafety::Safe };

    /// Returns `true` iff this carries the `safe` overflow-position modifier.
    #[inline]
    pub const fn is_safe(self) -> bool {
        matches!(self.safety, AlignmentSafety::Safe)
    }

    /// Returns this alignment with the given overflow-position modifier.
    #[inline]
    pub const fn with_safety(self, safety: AlignmentSafety) -> Self {
        Self { keyword: self.keyword, safety }
    }

    /// Returns the underlying position keyword, discarding the safety modifier.
    #[inline]
    pub const fn keyword(self) -> AlignContentKeyword {
        self.keyword
    }
}



/// Sets the distribution of space between and around content items.
/// For Flexbox it controls alignment in the main axis.
/// For Grid it controls alignment in the inline axis.
///
/// [MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/justify-content)
pub type JustifyContent = AlignContent;

// ---------------------------------------------------------------------------
// Serde — custom impls preserve the pre-struct wire format (single tag string
// per public spelling) so consumers reading data serialized before the refactor
// continue to deserialize correctly.
// ---------------------------------------------------------------------------







