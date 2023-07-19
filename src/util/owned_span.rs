use tui::{
    text::{Span, Spans, Text},
    style::{Style},
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Default)]
pub struct OwnedSpan {
    pub content: String,
    pub style: Style,
}

#[derive(Debug, Clone, Default)]
pub struct OwnedSpans(pub Vec<OwnedSpan>);

impl OwnedSpan {
    pub fn width(&self) -> usize {
        self.content.width()
    }

    pub fn raw<T: Into<String>>(text: T) -> Self {
        Self {
            content: text.into(),
            style: Style::default(),
        }
    }

    pub fn styled<T: Into<String>>(text: T, style: Style) -> Self {
        Self {
            content: text.into(),
            style,
        }
    }
}

impl OwnedSpans {
    pub fn width(&self) -> usize {
        self.0.iter().fold(0, |acc, s| acc + s.width())
    }
}

impl<'a> From<&'a str> for OwnedSpan {
    fn from(text: &'a str) -> OwnedSpan {
        OwnedSpan {
            content: text.to_string(),
            style: Style::default(),
        }
    }
}

impl<'a> From<Span<'a>> for OwnedSpan {
    fn from(span: Span<'a>) -> OwnedSpan {
        OwnedSpan {
            content: span.content.to_string(),
            style: span.style,
        }
    }
}

impl<'a> From<OwnedSpan> for Span<'a> {
    fn from(span: OwnedSpan) -> Span<'a> {
        Span::styled(span.content, span.style)
    }
}

impl<'a> From<&'a str> for OwnedSpans {
    fn from(text: &'a str) -> OwnedSpans {
        OwnedSpans::from(OwnedSpan::from(text))
    }
}

impl From<OwnedSpan> for OwnedSpans {
    fn from(span: OwnedSpan) -> OwnedSpans {
        OwnedSpans(vec![span])
    }
}

impl From<Vec<OwnedSpan>> for OwnedSpans {
    fn from(spans: Vec<OwnedSpan>) -> OwnedSpans {
        OwnedSpans(spans)
    }
}

impl<'a> From<Span<'a>> for OwnedSpans {
    fn from(span: Span<'a>) -> OwnedSpans {
        OwnedSpans::from(OwnedSpan::from(span))
    }
}

impl<'a> From<Spans<'a>> for OwnedSpans {
    fn from(spans: Spans<'a>) -> OwnedSpans {
        let spans = spans.0.into_iter().map(OwnedSpan::from).collect();
        OwnedSpans(spans)
    }
}

impl<'a> From<OwnedSpans> for Spans<'a> {
    fn from(spans: OwnedSpans) -> Spans<'a> {
        let spans = spans.0.into_iter().map(Span::from).collect();
        Spans(spans)
    }
}

impl<'a> From<OwnedSpans> for Text<'a> {
    fn from(spans: OwnedSpans) -> Text<'a> {
        Text::from(Spans::from(spans))
    }
}
