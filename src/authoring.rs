//! Authoring cluster（ARCHITECTURE.md §4/D4）：把页面和文档变成制品——
//! PDF/PNG 页泵（pages）、时间线视频（video）、OOXML 容器（ooxml）、
//! 原生 PPTX（pptx_native）、docgen 确定性 HTML。
//!
//! crate 拆分（diting-authoring）仍是 soft-defer；screenshot feature 是
//! D4 的便宜杠杆——四个渲染件全挂在这道门下，docgen 纯文本无需渲染面。
//! 2026-11-05 从 src/ 根平铺归拢到此（结构整改 #3），行为零变化。

pub mod docgen;
#[cfg(feature = "screenshot")]
pub mod ooxml;
#[cfg(feature = "screenshot")]
pub mod pages;
#[cfg(feature = "screenshot")]
pub mod pptx_native;
#[cfg(feature = "screenshot")]
pub mod video;
