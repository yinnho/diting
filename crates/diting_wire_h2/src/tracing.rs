macro_rules! debug {
    ($($arg:tt)+) => {
        {
        }
    }
}

pub(crate) use debug;

macro_rules! trace {
    ($($arg:tt)*) => {
        {
        }
    }
}

pub(crate) use trace;

macro_rules! trace_span {
    ($($arg:tt)*) => {
        {
        }
    }
}

pub(crate) use trace_span;

macro_rules! _warn {
    ($($arg:tt)*) => {
        {
        }
    }
}

pub(crate) use _warn as warn;
