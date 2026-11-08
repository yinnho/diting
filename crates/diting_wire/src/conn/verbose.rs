use super::AsyncConnWithInfo;

/// Controls whether to enable verbose tracing for connections.
///
/// When enabled (with the `tracing` feature), connections are wrapped to log I/O operations for
/// debugging.
#[derive(Clone, Copy)]
pub struct Verbose(pub(super) bool);

impl Verbose {
    pub const OFF: Verbose = Verbose(false);

    #[inline(always)]
    pub(super) fn wrap<T>(&self, conn: T) -> Box<dyn AsyncConnWithInfo>
    where
        T: AsyncConnWithInfo + 'static,
    {

        Box::new(conn)
    }
}

