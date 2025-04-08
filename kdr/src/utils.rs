#[cfg(target_arch = "wasm32")]
pub fn browser_console_log(msg: &str) {
    use wasm_bindgen::JsValue;

    web_sys::console::log_1(&JsValue::from_str(msg));
}

#[macro_export]
macro_rules! err {
    ($e: ident) => {{
        use eyre::eyre;

        Err(eyre!($e))
    }};

    ($format_string: literal) => {{
        use eyre::eyre;

        Err(eyre!($format_string))
    }};

    ($($arg:tt)*) => {{
        use eyre::eyre;

        Err(eyre!($($arg)*))
    }};
}
