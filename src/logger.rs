use std::fmt;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    fmt::{
        FmtContext,
        format::{self, FormatEvent, FormatFields},
    },
    registry::LookupSpan,
};

macro_rules! rgb {
    ($r:literal, $g:literal, $b:literal) => {
        concat!("\x1b[38;2;", $r, ";", $g, ";", $b, "m")
    };
}

const COLOR_TRACE: &str = rgb!(168, 230, 207);
const COLOR_DEBUG: &str = rgb!(189, 224, 254);
const COLOR_INFO: &str = rgb!(241, 138, 131);
const COLOR_WARN: &str = rgb!(255, 180, 162);
const COLOR_FAIL: &str = rgb!(239, 35, 60);
const COLOR_TEXT: &str = rgb!(226, 226, 226);
const RESET: &str = "\x1b[0m";

pub struct LogFormatter;

impl<S, N> FormatEvent<S, N> for LogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        context: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let level = *event.metadata().level();

        let (color, label) = match level {
            Level::TRACE => (COLOR_TRACE, "trace"),
            Level::DEBUG => (COLOR_DEBUG, "debug"),
            Level::INFO => (COLOR_INFO, "info "),
            Level::WARN => (COLOR_WARN, "warn "),
            Level::ERROR => (COLOR_FAIL, "fail "),
        };

        write!(writer, "{color}{label}{RESET} {COLOR_TEXT}")?;
        context.field_format().format_fields(writer.by_ref(), event)?;
        write!(writer, "{RESET}")?;

        writeln!(writer)
    }
}
