/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::borrow::ToOwned;
use std::collections::HashSet;
use std::mem;

use crate::Node;

/// The primary types of values that an XPath expression returns as a result.
#[derive(Debug)]
pub enum Value<N: Node> {
    Boolean(bool),
    /// A IEEE-754 double-precision floating point number.
    Number(f64),
    String(String),
    NodeSet(NodeSet<N>),
}

#[derive(Debug)]
pub struct NodeSet<N: Node> {
    nodes: Vec<N>,
    is_sorted: bool,
}

impl<N: Node> Default for NodeSet<N> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            is_sorted: false,
        }
    }
}

impl<N: Node> NodeSet<N> {
    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub(crate) fn push(&mut self, node: N) {
        self.is_sorted = false;
        self.nodes.push(node);
    }

    pub(crate) fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = N>,
    {
        self.nodes.extend(iter);
    }

    /// Whether this set is known to be sorted in tree order.
    ///
    /// This method is pessimistic and will never look at the elements in the set.
    /// As such, it *may* return `false` even if the set happens to be sorted.
    pub(crate) fn is_sorted(&self) -> bool {
        self.is_sorted || self.nodes.len() < 2
    }

    /// Assume that this set is sorted, without actually sorting it.
    pub(crate) fn assume_sorted(&mut self) {
        debug_assert!(
            self.nodes
                .is_sorted_by(|a, b| a.compare_tree_order(b).is_le())
        );
        self.is_sorted = true;
    }

    pub(crate) fn sort(&mut self) {
        if self.is_sorted() {
            return;
        }

        // Using sort_unstable_by here is fine because duplicates won't appear in the final
        // result anyways.
        self.nodes.sort_unstable_by(|a, b| a.compare_tree_order(b));
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &N> {
        self.nodes.iter()
    }

    /// Return the first node in tree order that appears within this set.
    ///
    /// This method will not sort the set itself.
    pub(crate) fn first(&self) -> Option<N> {
        if self.is_sorted() {
            return self.nodes.first().cloned();
        }

        self.iter().min_by(|a, b| a.compare_tree_order(b)).cloned()
    }

    pub(crate) fn deduplicate(&mut self) {
        let mut seen = HashSet::new();
        self.nodes = mem::take(&mut self.nodes)
            .into_iter()
            .filter_map(|node| {
                let opaque = node.to_opaque();
                seen.insert(opaque).then_some(node)
            })
            .collect();
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` for which `f(&e)` returns `false`.
    /// This method operates in place, visiting each element exactly once in the
    /// original order, and preserves the order of the retained elements.
    pub(crate) fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&N) -> bool,
    {
        self.nodes.retain(f)
    }

    pub(crate) fn reverse(&mut self) {
        self.nodes = mem::take(&mut self.nodes).into_iter().rev().collect();
    }
}

impl<N: Node> IntoIterator for NodeSet<N> {
    type IntoIter = <Vec<N> as IntoIterator>::IntoIter;
    type Item = <Vec<N> as IntoIterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

impl<N: Node> FromIterator<N> for NodeSet<N> {
    fn from_iter<T: IntoIterator<Item = N>>(iter: T) -> Self {
        Self {
            nodes: iter.into_iter().collect(),
            is_sorted: false,
        }
    }
}

/// Blink-flavoured XPath number-to-string (observed Chrome behaviour, not the
/// XPath 1.0 spec text): six significant digits; scientific notation with a
/// non-padded exponent when the decimal exponent is >= 6 or <= -7, plain
/// decimal with trailing zeros stripped otherwise; scientific keeps all six
/// significant digits including trailing zeros (e.g. "1.00000e+6").
pub(crate) fn xpath_number_to_string(number: f64) -> String {
    if number.is_nan() {
        return "NaN".to_owned();
    }
    if number.is_infinite() {
        return if number.is_sign_negative() { "-Infinity" } else { "Infinity" }.to_owned();
    }
    if number == 0.0 {
        return "0".to_owned();
    }
    let exp = number.abs().log10().floor() as i32;
    // correct for log10 edge effects by probing the rounded 6-sig-digit value
    if !(6..).contains(&exp) && exp > -7 {
        // decimal: round to 6 significant digits then strip trailing zeros
        let digits_after = 5 - exp;
        let factor = 10f64.powi(digits_after);
        let rounded = (number * factor).round() / factor;
        let mut out = format!("{rounded:.prec$}", prec = digits_after.max(0) as usize);
        if out.contains('.') {
            while out.ends_with('0') {
                out.pop();
            }
            if out.ends_with('.') {
                out.pop();
            }
        }
        // rounding may push across the scientific threshold (999999.5 -> 1e6)
        if out.len() - out.find('.').unwrap_or(out.len()) > 0 && out.trim_start_matches('-').replace('.', "").len() > 6 && exp == 5 {
            // rare carry case: fall through to scientific below using original number
        } else {
            return out;
        }
    }
    // scientific: mantissa with 5 decimals (trailing zeros kept), exponent unpadded
    let exp10 = number.abs().log10().floor() as i32;
    let mantissa = number / 10f64.powi(exp10);
    let mantissa = (mantissa * 100000.0).round() / 100000.0;
    let (mantissa, exp10) = if mantissa.abs() >= 10.0 { (mantissa / 10.0, exp10 + 1) } else { (mantissa, exp10) };
    let sign = if exp10 < 0 { "-" } else { "+" };
    format!("{mantissa:.5}e{sign}{}", exp10.abs())
}

pub(crate) fn parse_number_from_string(string: &str) -> f64 {
    // https://www.w3.org/TR/1999/REC-xpath-19991116/#function-number:
    // > a string that consists of optional whitespace followed by an optional minus sign followed
    // > by a Number followed by whitespace is converted to the IEEE 754 number that is nearest
    // > (according to the IEEE 754 round-to-nearest rule) to the mathematical value represented
    // > by the string; any other string is converted to NaN

    // The specification does not define what "whitespace" means exactly, we choose to trim only ascii whitespace,
    // as that seems to be what other browsers do.
    string.trim_ascii().parse().unwrap_or(f64::NAN)
}

/// Helper for `PartialEq<Value>` implementations
fn num_vals<N: Node>(nodes: &NodeSet<N>) -> Vec<f64> {
    nodes
        .iter()
        .map(|node| parse_number_from_string(&node.text_content()))
        .collect()
}

impl<N: Node> PartialEq<Value<N>> for Value<N> {
    fn eq(&self, other: &Value<N>) -> bool {
        match (self, other) {
            (Value::NodeSet(left_nodes), Value::NodeSet(right_nodes)) => {
                let left_strings: HashSet<String> =
                    left_nodes.iter().map(|node| node.text_content()).collect();
                let right_strings: HashSet<String> =
                    right_nodes.iter().map(|node| node.text_content()).collect();
                !left_strings.is_disjoint(&right_strings)
            }
            (&Value::NodeSet(ref nodes), &Value::Number(val))
            | (&Value::Number(val), &Value::NodeSet(ref nodes)) => {
                let numbers = num_vals(nodes);
                numbers.contains(&val)
            }
            (&Value::NodeSet(ref nodes), &Value::String(ref string))
            | (&Value::String(ref string), &Value::NodeSet(ref nodes)) => nodes
                .iter()
                .map(|node| node.text_content())
                .any(|text_content| &text_content == string),
            (&Value::Boolean(_), _) | (_, &Value::Boolean(_)) => {
                self.convert_to_boolean() == other.convert_to_boolean()
            }
            (&Value::Number(_), _) | (_, &Value::Number(_)) => {
                self.convert_to_number() == other.convert_to_number()
            }
            _ => self.convert_to_string() == other.convert_to_string(),
        }
    }
}

impl<N: Node> Value<N> {
    /// XPath 1.0 §3.4: `!=` is not the negation of `=` for node-sets — it is
    /// true when SOME pairing is unequal (so empty node-sets make both `=`
    /// and `!=` false). Scalar/scalar cases are plain inequality after the
    /// same conversion rules `eq` uses.
    pub(crate) fn xpath_ne(&self, other: &Value<N>) -> bool {
        match (self, other) {
            (Value::NodeSet(left_nodes), Value::NodeSet(right_nodes)) => {
                let left_strings: HashSet<String> =
                    left_nodes.iter().map(|node| node.text_content()).collect();
                let right_strings: HashSet<String> =
                    right_nodes.iter().map(|node| node.text_content()).collect();
                left_strings.iter().any(|l| !right_strings.contains(l))
                    || right_strings.iter().any(|r| !left_strings.contains(r))
            }
            (&Value::NodeSet(ref nodes), &Value::Number(val))
            | (&Value::Number(val), &Value::NodeSet(ref nodes)) => {
                let numbers = num_vals(nodes);
                numbers.iter().any(|n| n != &val)
            }
            (&Value::NodeSet(ref nodes), &Value::String(ref string))
            | (&Value::String(ref string), &Value::NodeSet(ref nodes)) => nodes
                .iter()
                .map(|node| node.text_content())
                .any(|text_content| &text_content != string),
            (&Value::Boolean(_), _) | (_, &Value::Boolean(_)) => {
                self.convert_to_boolean() != other.convert_to_boolean()
            }
            (&Value::Number(_), _) | (_, &Value::Number(_)) => {
                self.convert_to_number() != other.convert_to_number()
            }
            _ => self.convert_to_string() != other.convert_to_string(),
        }
    }
}

impl<N: Node> Value<N> {
    /// <https://www.w3.org/TR/1999/REC-xpath-19991116/#function-boolean>
    pub fn convert_to_boolean(&self) -> bool {
        match self {
            Value::Boolean(boolean) => *boolean,
            Value::Number(number) => *number != 0.0 && !number.is_nan(),
            Value::String(string) => !string.is_empty(),
            Value::NodeSet(nodeset) => !nodeset.is_empty(),
        }
    }

    /// <https://www.w3.org/TR/1999/REC-xpath-19991116/#function-number>
    pub fn convert_to_number(&self) -> f64 {
        match self {
            Value::Boolean(boolean) => {
                if *boolean {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(number) => *number,
            Value::String(string) => parse_number_from_string(string),
            Value::NodeSet(_) => parse_number_from_string(&self.convert_to_string()),
        }
    }

    /// <https://www.w3.org/TR/1999/REC-xpath-19991116/#function-string>
    pub fn convert_to_string(&self) -> String {
        match self {
            Value::Boolean(value) => value.to_string(),
            Value::Number(number) => {
                if number.is_infinite() {
                    if number.is_sign_negative() {
                        "-Infinity".to_owned()
                    } else {
                        "Infinity".to_owned()
                    }
                } else if *number == 0.0 {
                    // catches -0.0 also
                    "0".into()
                } else {
                    xpath_number_to_string(*number)
                }
            }
            Value::String(string) => string.to_owned(),
            Value::NodeSet(nodes) => nodes
                .first()
                .as_ref()
                .map(Node::text_content)
                .unwrap_or_default(),
        }
    }
}

macro_rules! from_impl {
    ($raw:ty, $variant:expr) => {
        impl<N: Node> From<$raw> for Value<N> {
            fn from(other: $raw) -> Self {
                $variant(other)
            }
        }
    };
}

from_impl!(bool, Value::Boolean);
from_impl!(f64, Value::Number);
from_impl!(String, Value::String);
impl<'a, N: Node> From<&'a str> for Value<N> {
    fn from(other: &'a str) -> Self {
        Value::String(other.into())
    }
}

macro_rules! partial_eq_impl {
    ($raw:ty, $variant:pat => $b:expr) => {
        impl<N: Node> PartialEq<$raw> for Value<N> {
            fn eq(&self, other: &$raw) -> bool {
                match *self {
                    $variant => $b == other,
                    _ => false,
                }
            }
        }

        impl<N: Node> PartialEq<Value<N>> for $raw {
            fn eq(&self, other: &Value<N>) -> bool {
                match *other {
                    $variant => $b == self,
                    _ => false,
                }
            }
        }
    };
}

partial_eq_impl!(bool, Value::Boolean(ref v) => v);
partial_eq_impl!(f64, Value::Number(ref v) => v);
partial_eq_impl!(String, Value::String(ref v) => v);
partial_eq_impl!(&str, Value::String(ref v) => v);

#[cfg(test)]
mod tests {

    use crate::dummy_implementation;

    type Value = super::Value<dummy_implementation::DummyNode>;

    #[test]
    fn string_value_to_number() {
        assert_eq!(Value::String("42.123".into()).convert_to_number(), 42.123);
        assert_eq!(Value::String(" 42\n".into()).convert_to_number(), 42.);
        assert!(
            Value::String("totally-invalid".into())
                .convert_to_number()
                .is_nan()
        );

        // U+2004 is non-ascii whitespace, which should be rejected
        assert!(
            Value::String("\u{2004}42".into())
                .convert_to_number()
                .is_nan()
        );
    }

    #[test]
    fn number_value_to_string() {
        assert_eq!(Value::Number(f64::NAN).convert_to_string(), "NaN");
        assert_eq!(Value::Number(0.).convert_to_string(), "0");
        assert_eq!(Value::Number(-0.).convert_to_string(), "0");
        assert_eq!(Value::Number(f64::INFINITY).convert_to_string(), "Infinity");
        assert_eq!(
            Value::Number(f64::NEG_INFINITY).convert_to_string(),
            "-Infinity"
        );
        assert_eq!(Value::Number(42.0).convert_to_string(), "42");
        assert_eq!(Value::Number(-42.0).convert_to_string(), "-42");
        assert_eq!(Value::Number(0.75).convert_to_string(), "0.75");
        assert_eq!(Value::Number(-0.75).convert_to_string(), "-0.75");
    }

    #[test]
    fn boolean_value_to_string() {
        assert_eq!(Value::Boolean(false).convert_to_string(), "false");
        assert_eq!(Value::Boolean(true).convert_to_string(), "true");
    }
}
