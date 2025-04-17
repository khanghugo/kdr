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

#[cfg(target_arch = "wasm32")]
pub fn spawn_async<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_async<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
    F::Output: Send + 'static,
{
    std::thread::spawn(move || {
        futures::executor::block_on(future);
    });
}
