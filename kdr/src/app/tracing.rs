use std::{env, fs::OpenOptions, sync::Once};

use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

// from bxt-rs src/utils/mod.rs
// https://github.com/YaLTeR/bxt-rs/blob/608fa9ff4b0ebdece1acd975065efb586ba15e1b/src/utils/mod.rs#L54

#[cfg(not(target_arch = "wasm32"))]
fn setup_logging_hooks() {
    let only_message = tracing_subscriber::fmt::format::debug_fn(|writer, field, value| {
        if field.name() == "message" {
            write!(writer, "{value:?}")
        } else {
            Ok(())
        }
    });

    let term_layer = tracing_subscriber::fmt::layer().fmt_fields(only_message.clone());

    // Disable ANSI colors on Windows as they don't work properly in the legacy console window.
    // https://github.com/tokio-rs/tracing/issues/445
    #[cfg(windows)]
    let term_layer = term_layer.with_ansi(false);

    let file_layer = OpenOptions::new()
        .append(true)
        .create(true)
        .open("kdr.log")
        .ok()
        .map(|file| {
            tracing_subscriber::fmt::layer()
                .with_writer(file)
                .with_ansi(false)
        });

    let profiling_layer = if env::var_os("KDR_PROFILE").is_some() {
        let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .file("trace.json")
            .include_args(true)
            .include_locations(false)
            .build();

        Box::leak(Box::new(guard));

        Some(chrome_layer)
    } else {
        None
    };

    #[cfg(feature = "tracing-tracy")]
    let tracy_layer = if env::var_os("KDR_PROFILE_TRACY").is_some() {
        struct TracyLayerConfig<F>(F);

        impl<F> tracing_tracy::Config for TracyLayerConfig<F>
        where
            F: for<'writer> tracing_subscriber::fmt::FormatFields<'writer> + 'static,
        {
            type Formatter = F;

            fn formatter(&self) -> &Self::Formatter {
                &self.0
            }
        }

        let config = TracyLayerConfig(only_message);
        Some(tracing_tracy::TracyLayer::new(config))
    } else {
        None
    };

    #[cfg(not(feature = "tracing-tracy"))]
    let tracy_layer = None::<tracing_subscriber::layer::Identity>;

    let level_filter = if env::var_os("KDR_VERBOSE").is_some() {
        LevelFilter::TRACE
    } else {
        LevelFilter::DEBUG
    };

    let env_filter = EnvFilter::new(
        "debug,naga=off,wgpu_hal=off,symphonia_core=off,wgpu_core::device::global=off",
    );

    tracing_subscriber::registry()
        .with(level_filter)
        .with(env_filter)
        .with(file_layer)
        .with(profiling_layer)
        .with(tracy_layer)
        // Term layer must be last, otherwise the log file will have some ANSI codes:
        // https://github.com/tokio-rs/tracing/issues/1817
        .with(term_layer)
        .init();
}

#[cfg(target_arch = "wasm32")]
fn setup_logging_hooks() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    tracing_wasm::set_as_global_default();

    tracing_wasm::WASMLayerConfigBuilder::new()
        .set_max_level(tracing::Level::DEBUG)
        .build();
}

pub fn ensure_logging_hooks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(setup_logging_hooks);

    info!("hello tracing")
}
