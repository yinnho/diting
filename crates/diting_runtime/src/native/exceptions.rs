//! DOMException as a native interface (WebIDL "exception" interface).
//! No instance state beyond name/message; `code` is derived from `name`.
//! Instances are NOT created by the Error constructor, so they carry no
//! own `stack` - matching Chrome's DOMException (probe: dex.ownkeys, dex.stack).

use super::declare::{Attr, Def, Iface, Member, Slot, Val};

/// Legacy error-code table (WebIDL DOMException names -> codes).
fn code_of(name: &str) -> i32 {
    match name {
        "IndexSizeError" => 1,
        "HierarchyRequestError" => 3,
        "WrongDocumentError" => 4,
        "InvalidCharacterError" => 5,
        "NoModificationAllowedError" => 7,
        "NotFoundError" => 8,
        "NotSupportedError" => 9,
        "InUseAttributeError" => 10,
        "InvalidStateError" => 11,
        "SyntaxError" => 12,
        "InvalidModificationError" => 13,
        "NamespaceError" => 14,
        "InvalidAccessError" => 15,
        "TypeMismatchError" => 17,
        "SecurityError" => 18,
        "NetworkError" => 19,
        "AbortError" => 20,
        "URLMismatchError" => 21,
        "QuotaExceededError" => 22,
        "TimeoutError" => 23,
        "InvalidNodeTypeError" => 24,
        "DataCloneError" => 25,
        _ => 0,
    }
}

pub(crate) static DOM_EXCEPTION: Iface = Iface {
    name: "DOMException", parent: None, family: 0x0E01,
    // ctor(message = "", name = "Error")
    slots: &[Slot { name: "message", default: Def::Str("") }, Slot { name: "name", default: Def::Str("Error") }],
    members: &[
        Member { name: "code", attr: Attr::Derived(|v| Val::Num(code_of(v[1].str()) as f64)), writable: false },
        Member { name: "name", attr: Attr::Slot(1), writable: false },
        Member { name: "message", attr: Attr::Slot(0), writable: false },
    ],
    to_json: false, factory: None, error_like: true,
    consts: &[
        ("INDEX_SIZE_ERR", 1),
        ("DOMSTRING_SIZE_ERR", 2),
        ("HIERARCHY_REQUEST_ERR", 3),
        ("WRONG_DOCUMENT_ERR", 4),
        ("INVALID_CHARACTER_ERR", 5),
        ("NO_DATA_ALLOWED_ERR", 6),
        ("NO_MODIFICATION_ALLOWED_ERR", 7),
        ("NOT_FOUND_ERR", 8),
        ("NOT_SUPPORTED_ERR", 9),
        ("INUSE_ATTRIBUTE_ERR", 10),
        ("INVALID_STATE_ERR", 11),
        ("SYNTAX_ERR", 12),
        ("INVALID_MODIFICATION_ERR", 13),
        ("NAMESPACE_ERR", 14),
        ("INVALID_ACCESS_ERR", 15),
        ("VALIDATION_ERR", 16),
        ("TYPE_MISMATCH_ERR", 17),
        ("SECURITY_ERR", 18),
        ("NETWORK_ERR", 19),
        ("ABORT_ERR", 20),
        ("URL_MISMATCH_ERR", 21),
        ("QUOTA_EXCEEDED_ERR", 22),
        ("TIMEOUT_ERR", 23),
        ("INVALID_NODE_TYPE_ERR", 24),
        ("DATA_CLONE_ERR", 25),
    ],
};
