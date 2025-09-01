pub trait ErrorReport: std::error::Error {
    /// Returns a string representation of the error and its source chain.
    fn as_report(&self) -> String {
        use std::fmt::Write;
        let mut report = self.to_string();

        // SAFETY: write! is suggested by clippy, and is trivially safe usage.
        std::iter::successors(self.source(), |child| child.source())
            .for_each(|source| write!(report, "\ncaused by: {source}").unwrap());

        report
    }
}

impl<T: std::error::Error> ErrorReport for T {}
