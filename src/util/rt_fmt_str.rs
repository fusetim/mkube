use core::fmt::{Debug, Display, Error, Formatter, Result};
use rt_format::argument::FormatArgument;
use rt_format::{Format, Specifier};
use std::borrow::Cow;

pub struct FmtStr<'a>(Cow<'a, str>);

impl<'a> FmtStr<'a> {
    pub fn new<T: Into<Cow<'a, str>>>(val: T) -> Self {
        Self(val.into())
    }
}

impl<'a> FormatArgument for FmtStr<'a> {
    fn supports_format(&self, specifier: &Specifier) -> bool {
        specifier.format == Format::Debug || specifier.format == Format::Display
    }
    fn fmt_display(&self, f: &mut Formatter<'_>) -> Result {
        return Display::fmt(&self.0, f);
    }
    fn fmt_debug(&self, f: &mut Formatter<'_>) -> Result {
        return Debug::fmt(&self.0, f);
    }
    fn fmt_octal(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
    fn fmt_lower_hex(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
    fn fmt_upper_hex(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
    fn fmt_binary(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
    fn fmt_lower_exp(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
    fn fmt_upper_exp(&self, _f: &mut Formatter<'_>) -> Result {
        return Err(Error);
    }
}
