use crossterm::event::KeyCode;
use std::path::PathBuf;
use std::borrow::Cow;
use tui::{
    buffer::Buffer,
    text::{Span, Spans, Text},
    layout::{Layout, Constraint, Rect, Direction},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget, Block, Wrap, Borders, BorderType},
};

use crate::nfo::Movie;
use crate::views::movie_manager::{MovieManagerEvent, MovieManagerMessage};
use crate::MESSAGE_SENDER;
use crate::{AppEvent, AppMessage};

#[derive(Clone, Debug, PartialEq)]
pub struct MovieDetails<'a> {
    pub movie: &'a Movie,
}

impl<'a> Widget for MovieDetails<'a> {

    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(format!(" {} ", self.movie.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(4), Constraint::Percentage(100)])
            .split(block.inner(area.clone()));
        let label_style = Style::default().fg(Color::LightYellow);
        let value_style = Style::default().fg(Color::Gray);
        let mut media_value = String::new();
        if let Some(fi) = &self.movie.fileinfo {
            if let Some(vt) = fi.streamdetails.video.get(0) {
                media_value+=&vt.codec;
                if let Some(res) = vt.height {
                    media_value = format!("{} {}p", &media_value, res);
                }
            }
            let mut tmpcodec = String::new();
            let mut tmplang = String::new();
            for at in &fi.streamdetails.audio {
                tmpcodec = if tmpcodec.len() == 0 { at.codec.to_owned() } else {tmpcodec+"/"+&at.codec};
                tmplang = if tmplang.len() == 0 { at.language.as_deref().unwrap_or("unk").into() } else {tmplang+"/"+at.language.as_deref().unwrap_or("unk")};
            }
            if tmpcodec.len()+tmplang.len() > 0 {
                media_value = if media_value.len() == 0 { format!("{} ({})", &tmpcodec, &tmplang) } else { media_value + " + " + &tmpcodec + " (" + &tmplang + ")"};
            }
            tmpcodec.clear();
            tmplang.clear();
            for st in &fi.streamdetails.subtitle {
                tmpcodec = if tmpcodec.len() == 0 { st.codec.as_deref().unwrap_or("unk").into() } else {tmpcodec+"/"+st.codec.as_deref().unwrap_or("unk")};
                tmplang = if tmplang.len() == 0 { st.language.as_deref().unwrap_or("unk").into() } else {tmplang+"/"+st.language.as_deref().unwrap_or("unk")};
            }
            if tmpcodec.len()+tmplang.len() > 0 {
                media_value = if media_value.len() == 0 { format!("{} ({})", &tmpcodec, &tmplang) } else { media_value + " + " + &tmpcodec + " (" + &tmplang + ")"};
            }
        } else { media_value+= " N / A "};
        let content = vec![
            Spans::from(vec![
                Span::styled("Release date: ", label_style),
                Span::styled(self.movie.premiered.as_deref().unwrap_or("   N/A    "), value_style),
                Span::raw("    "),
                Span::styled("Duration: ", label_style),
                Span::styled(self.movie.runtime.map(format_duration).unwrap_or(" N/A ".into()), value_style),
                Span::raw("    "),
                Span::styled("Country: ", label_style),
                Span::styled(self.movie.country.join(", "), value_style),
            ]),
            Spans::from(vec![
                Span::styled("Genre: ", label_style),
                Span::styled(self.movie.genre.join(", "), value_style),
                Span::raw("    "),
                Span::styled("Director: ", label_style),
                Span::styled(self.movie.director.iter().take(2).map(|d| Cow::from(&d.name)).reduce(|acc,d| acc+", "+d).unwrap_or("N/A".into()), value_style),
            ]),
            Spans::from(vec![
                Span::styled("Production: ", label_style),
                Span::styled(self.movie.studio.iter().take(4).map(Cow::from).reduce(|acc,d| acc+", "+d).unwrap_or("N/A".into()), value_style),
            ]),
            Spans::from(vec![
                Span::styled("Media: ", label_style),
                Span::styled(media_value, value_style),
                Span::raw("    "),
                Span::styled("Source: ", label_style),
                Span::styled(format!("{:^6}", self.movie.source.as_deref().unwrap_or("N/A")), value_style),
            ]),
        ];
        let plot = Spans::from(vec![
            Span::styled("Plot: ", label_style),
            Span::styled(self.movie.plot.as_deref().unwrap_or("None"), value_style),
        ]);
        block.render(area, buf);
        Paragraph::new(content).wrap(Wrap { trim: true }).render(chunks[0], buf);
        Paragraph::new(plot).wrap(Wrap { trim: true }).render(chunks[1], buf);
    }
}

fn format_duration(minutes: u64) -> String {
    let hours = minutes / 60;
    let rem_minutes = minutes % 60;
    if hours > 0 {
        format!("{:2>}h{:02}", hours, rem_minutes)
    } else {
        format!("{:2>}min", rem_minutes)
    }
}