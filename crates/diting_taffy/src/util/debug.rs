#![allow(dead_code)]





macro_rules! debug_log {
    // String literal label with debug printing
    ($label:literal, dbg:$item:expr) => {{
    }};
    // String literal label with display printing
    ($label:literal, $item:expr) => {{
    }};
    // Debug printing
    (dbg:$item:expr) => {{
    }};
    // Display printing
    ($item:expr) => {{
    }};
    // Blank newline
    () => {{
    }};
}

macro_rules! debug_log_node {
    ($inputs: expr) => {
        debug_log!(dbg:$inputs.run_mode);
        debug_log!("sizing_mode", dbg:$inputs.sizing_mode);
        debug_log!("known_dimensions", dbg:$inputs.known_dimensions);
        debug_log!("parent_size", dbg:$inputs.parent_size);
        debug_log!("available_space", dbg:$inputs.available_space);
    };
}

macro_rules! debug_push_node {
    ($node_id:expr) => {
        debug_log!("");
    };
}

macro_rules! debug_pop_node {
    () => {
    };
}


#[allow(unused_macros)]
macro_rules! time {
    ($label:expr, $($code:tt)*) => {
        $($code)*
    };
}

#[allow(unused_imports)]
pub(crate) use {debug_log, debug_log_node, debug_pop_node, debug_push_node, time};
