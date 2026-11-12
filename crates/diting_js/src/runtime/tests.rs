#![allow(clippy::await_holding_lock)] // net tests hold PRIVATE_NET_ENV_LOCK across awaits on purpose — the guard IS the serialization (1f7486c)

// 测试已按域分桶（docs/tests-分桶表-20261113.md，#74）；新测试进对应域桶，hub 只挂声明。
#[cfg(test)]
mod bridge;

#[cfg(test)]
mod canvas;

#[cfg(test)]
mod common;

#[cfg(test)]
mod cookies;

#[cfg(test)]
mod crypto;

#[cfg(test)]
mod css;

#[cfg(test)]
mod css_anim;

#[cfg(test)]
mod css_computed;

#[cfg(test)]
mod document;

#[cfg(test)]
mod dom_shadow;

#[cfg(test)]
mod dom_tree;

#[cfg(test)]
mod events;

#[cfg(test)]
mod files;

#[cfg(test)]
mod forms;

#[cfg(test)]
mod iframes;

#[cfg(test)]
mod knife_a;

#[cfg(test)]
mod knife_b;

#[cfg(test)]
mod knife_c;

#[cfg(test)]
mod layout_flow;

#[cfg(test)]
mod layout_geometry;

#[cfg(test)]
mod layout_scroll;

#[cfg(test)]
mod location;

#[cfg(test)]
mod media;

#[cfg(test)]
mod mutation;

#[cfg(test)]
mod net_fetch;

#[cfg(test)]
mod net_policy;

#[cfg(test)]
mod net_xhr;

#[cfg(test)]
mod performance;

#[cfg(test)]
mod scripts;

#[cfg(test)]
mod stealth_env;

#[cfg(test)]
mod storage;

#[cfg(test)]
mod text;

#[cfg(test)]
mod timers;

#[cfg(test)]
mod webidl_faces;

#[cfg(test)]
mod window;

#[cfg(test)]
mod workers;
