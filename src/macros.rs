#![macro_use]

macro_rules! fail {
    ($expr:expr) => (
        return Err(::std::convert::From::from($expr));
    );
    ($expr:expr $(, $more:expr)+) => (
        return fail!(format!($expr, $($more),*))
    )
}
