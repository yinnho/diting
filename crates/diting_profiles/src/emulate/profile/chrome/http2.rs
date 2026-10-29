// HEADERS-frame priority. Every Chromium net stack sends exclusive, depends_on 0, wire weight
// 256 (the field holds weight - 1 = 255). Measured on tls.peet.ws with Chrome for Testing
// 145.0.7632.117 and 156.0.8078.73 (tools/webplatform/chrome/wire/). The upstream preset this
// was taken from shipped 219 (wire 220), a per-connection tell finer H2 fingerprints see.
macro_rules! headers_stream_dependency {
    () => {
        StreamDependency::new(StreamId::zero(), 255, true)
    };
}

macro_rules! pseudo_order {
    () => {
        PseudoOrder::builder()
            .extend([
                PseudoId::Method,
                PseudoId::Authority,
                PseudoId::Scheme,
                PseudoId::Path,
            ])
            .build()
    };
}

macro_rules! settings_order {
    () => {
        SettingsOrder::builder()
            .extend([
                SettingId::HeaderTableSize,
                SettingId::EnablePush,
                SettingId::MaxConcurrentStreams,
                SettingId::InitialWindowSize,
                SettingId::MaxFrameSize,
                SettingId::MaxHeaderListSize,
                SettingId::EnableConnectProtocol,
                SettingId::NoRfc7540Priorities,
            ])
            .build()
    };
}

macro_rules! http2_options {
    (@base $builder:expr) => {
        $builder
            .initial_window_size(6291456)
            .initial_connection_window_size(15728640)
            .max_header_list_size(262144)
            .header_table_size(65536)
            .headers_stream_dependency(headers_stream_dependency!())
            .headers_pseudo_order(pseudo_order!())
            .settings_order(settings_order!())
    };

    (1) => {
        http2_options!(@base Http2Options::builder())
            .max_concurrent_streams(1000)
            .build()
    };
    (2) => {
        http2_options!(@base Http2Options::builder())
            .max_concurrent_streams(1000)
            .enable_push(false)
            .build()
    };
    (3) => {
        http2_options!(@base Http2Options::builder())
            .enable_push(false)
            .build()
    };
    // (4) = (3) + a random GREASE setting per connection, as real Chrome 145 sends it.
    (4) => {
        http2_options!(@base Http2Options::builder())
            .enable_push(false)
            .grease_setting(true)
            .build()
    };
}
