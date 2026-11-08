//! Authoring cluster（ARCHITECTURE.md §4/D4）：把页面和文档变成制品——
//! PDF/PNG 页泵（pages）、时间线视频（video）、OOXML 容器（ooxml）、
//! 原生 PPTX（pptx_native）、docgen 确定性 HTML。
//!
//! 2026-11-08 从 diting-browser 抽成独立 crate，产品经 `authoring` feature
//! 可选挂载（默认开）。本 crate 的 `screenshot` feature 打开四个渲染件
//! （pages/video/ooxml/pptx_native，要驱动引擎布局/绘制）；docgen 是纯文本，
//! 不需要渲染面。

pub mod docgen;
#[cfg(feature = "screenshot")]
pub mod ooxml;
#[cfg(feature = "screenshot")]
pub mod pages;
#[cfg(feature = "screenshot")]
pub mod pptx_native;
#[cfg(feature = "screenshot")]
pub mod video;
