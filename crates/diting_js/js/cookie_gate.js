// document.cookie access gate, split out of bootstrap.js (JS-PLATFORM-ARCH).
// Stays a LEGACY-tier script on purpose: bootstrap's and iframe_realm's
// document.cookie accessors name __cookieGate as a free global-lexical binding.
// It leaves with the Document.cookie family when that moves to IDL-generated
// modules. Const, not a function declaration: mints no window property.

// document.cookie access gate: origins with no cookie jar (about:blank
// initial documents, data:) throw SecurityError in Chrome on BOTH read and
// write — with different prepositions — they never silently no-op (#274).
// The ops resolve against the session URL, which is also how same-origin
// and srcdoc iframes inherit the parent origin below.
const __cookieGate = function (setting) {
  var ok = false;
  try {
    var proto = new URL(_domParse("document_url")).protocol;
    ok = proto === 'http:' || proto === 'https:';
  } catch (e) {}
  if (!ok) {
    throw new DOMException(
      "Failed to " +
        (setting ? "set the 'cookie' property on 'Document'" : "read the 'cookie' property from 'Document'") +
        ": Access is denied for this document.",
      "SecurityError");
  }
};
