use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Arc, PoisonError};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use diting_dom::{AttachShadowError, DomTree, NodeData, NodeId, ShadowRootMode};
use diting_net::{CookieJar, HttpClient};
use diting_ops::diting_op;
use diting_runtime::state::OpState;
use url::Url;
pub(crate) use timers::op_sleep_body;
#[cfg(feature = "screenshot")]
pub use storage::store_image_bytes;
#[cfg(test)]
pub(crate) use storage::reset_local_storage_for_tests;
use storage::storage_file;
pub(crate) use fetch_ops::{FetchCredentials, FetchSettlements, glob_match, op_fetch_url_body, FETCH_REDIRECT_LIMIT};
use fetch_ops::REFERRER_POLICY_TOKENS;
#[cfg(feature = "screenshot")]
pub use layout_run::text_ink_extent;
pub use state::{InterceptResolution, InterceptedRequest, INTERCEPT_RESOLUTION_TIMEOUT_MS, ResourceTimingRecord, JsState, JsNetworkEvent, StoredNetworkResponseBody, SharedState};
pub(crate) use state::{BlobStoreMap, fetch_body_byte_limit};
use state::{response_body_entry_limit, response_body_byte_limit};
use script_start::{mark_script_subtree_started, propagate_script_start_state};
use dom::{parse_nid, DomCmd};
use dom_cmds::{dom_cmd_attr_state, dom_cmd_create, dom_cmd_document, dom_cmd_mutate, dom_cmd_query, dom_cmd_tree};
use dom_style::{dom_cmd_geometry, dom_cmd_scroll_style, dom_cmd_sheets};
#[cfg(feature = "screenshot")]
use dom_style::style_property_map;
#[cfg(feature = "screenshot")]
use layout_run::{ensure_layout_run, iframe_layout_run, is_element_scroller, is_viewport_fixed, parse_iframe_viewport, sticky_shifts, LayoutRun};
#[cfg(feature = "screenshot")]
use dom_style::{attr_write_is_layout_inert, style_write_is_paint_only};

mod band;
mod canvas;
mod css;
mod ecdsa;
mod ed25519;
mod encoding;
mod fetch_gate;
mod fetch_walk;
mod node_order;
mod props;
mod text_geom;

#[cfg(feature = "screenshot")]
pub use band::BandFrame;
#[cfg(feature = "screenshot")]
pub use band::{
    band_frame, band_frame_cut, band_frame_cut_with_text, band_frame_scaled, band_frame_with_text,
};
pub(crate) use canvas::{op_canvas_jpeg, op_canvas_png};
#[cfg(feature = "screenshot")]
pub(crate) use canvas::{op_canvas_store, op_canvas_text};
pub(crate) use ecdsa::{
    op_subtle_ecdh_derive_bits, op_subtle_ecdsa_generate, op_subtle_ecdsa_import_private,
    op_subtle_ecdsa_import_public, op_subtle_ecdsa_sign, op_subtle_ecdsa_verify,
};
pub(crate) use ed25519::{
    op_subtle_ed25519_check, op_subtle_ed25519_public, op_subtle_ed25519_sign,
    op_subtle_ed25519_verify,
};
pub(crate) use fetch_walk::{fetch_url_walk, FetchFailure, FetchNetworkEvent, FetchWalkDeps};
use node_order::{compare_node_order, node_child_index};
mod cors;
mod crypto_secret;
mod referrer;
use referrer::resolve_document_referrer_policy;
pub use referrer::{fetch_referer, last_valid_referrer_token, resolve_referrer_policy_from};
mod fetch_client;
#[cfg(feature = "screenshot")]
use cssom_value::{cb_inline_for, computed_style_value};
pub use fetch_client::cached_request_client;
pub(crate) use fetch_client::select_request_client;
mod cssom_value;
mod image_info;
pub(crate) use cors::request_origin;
pub(crate) use crypto_secret::{
    crypto_err, op_random_bytes, op_subtle_aes_cbc, op_subtle_aes_ctr, op_subtle_aes_gcm,
    op_subtle_aes_kw, op_subtle_digest, op_subtle_hkdf, op_subtle_hmac, pbkdf2_derive,
};
pub use fetch_gate::validate_fetch_url;
pub(crate) use image_info::op_image_info;
mod video_info;
pub(crate) use video_info::op_video_info;
mod crypto_rsa;
pub(crate) use crypto_rsa::{
    op_subtle_rsa_import_pkcs8, op_subtle_rsa_import_private, op_subtle_rsa_import_public,
    op_subtle_rsa_import_spki, op_subtle_rsa_pkcs1v15_verify, op_subtle_rsa_pss_verify,
    rsa_generate_flat, rsa_oaep, rsa_pkcs1v15_sign, rsa_pss_sign,
};

/// The op table our own host installs (10d flip): every #[diting_op]
/// wrapper already returns OUR OpDecl, and the four async ops come in as
/// hand glue from `diting_js::async_glue` (op_fetch_url / op_sleep /
/// op_ws_open / op_ws_next_message). The list mirrors the retired
/// build_extension() entry-for-entry; the to_deno adapter and the
/// Extension wrapper died with the deno host.
pub fn our_decls() -> Vec<diting_runtime::op_reg::OpDecl> {
    let mut decls = vec![
        css::op_css_supports(),
        props::op_register_property(),
        dom::op_dom(),
        timers::op_clock_ms(),
        svg::op_svg_path_len(),
        svg::op_svg_path_point(),
        dom::op_shadow_attach(),
        console::op_console_msg(),
        console::op_dialog(),
        script_start::op_script_mark_started(),
        script_start::op_script_try_start(),
        script_start::op_dyn_script_fetch_begin(),
        script_start::op_dyn_script_fetch_end(),
        storage::op_blob_register(),
        storage::op_blob_revoke(),
        fetch_ops::op_fetch_take(),
        fetch_ops::op_fetch_url_sync(),
        op_image_info(),
        op_video_info(),
        op_canvas_png(),
        op_canvas_jpeg(),
        #[cfg(feature = "screenshot")]
        op_canvas_store(),
        #[cfg(feature = "screenshot")]
        op_canvas_text(),
        storage::op_get_cookies(),
        storage::op_set_cookie(),
        storage::op_storage_read(),
        storage::op_storage_write(),
        nav::op_navigate(),
        nav::op_history_jump_to(),
        nav::op_binding_called(),
        op_subtle_digest(),
        op_subtle_hmac(),
        op_subtle_aes_gcm(),
        op_subtle_aes_cbc(),
        op_subtle_aes_ctr(),
        op_subtle_aes_kw(),
        op_subtle_rsa_import_public(),
        op_subtle_rsa_import_spki(),
        op_subtle_rsa_import_private(),
        op_subtle_rsa_import_pkcs8(),
        op_subtle_rsa_pkcs1v15_verify(),
        op_subtle_rsa_pss_verify(),
        op_subtle_hkdf(),
        op_subtle_ecdsa_generate(),
        op_subtle_ecdsa_import_private(),
        op_subtle_ecdsa_import_public(),
        op_subtle_ecdh_derive_bits(),
        op_subtle_ecdsa_sign(),
        op_subtle_ecdsa_verify(),
        op_subtle_ed25519_check(),
        op_subtle_ed25519_public(),
        op_subtle_ed25519_sign(),
        op_subtle_ed25519_verify(),
        op_random_bytes(),
        url_ops::op_url_parse(),
        url_ops::op_url_set(),
        url_ops::op_url_resolve(),
        crate::import_map::op_add_import_map(),
        encoding::op_encoding_for_label(),
        encoding::op_text_decode(),
        encoding::op_url_encode_query(),
        crate::ws::op_ws_send(),
        crate::ws::op_ws_close(),
    ];
    // The async four ride the hand glue (promise stub + OpDriver spawn).
    decls.extend(crate::async_glue::all_decls());
    decls
}

mod state;
mod script_start;
mod dom;
mod dom_cmds;
mod dom_style;
mod layout_run;
mod console;
mod fetch_ops;
mod storage;
mod nav;
mod timers;
mod url_ops;
mod svg;
#[cfg(test)]
mod tests;
