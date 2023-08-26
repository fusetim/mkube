use anyhow::{anyhow, Context, Result};
use futures_util::stream::StreamExt;
use remotefs::fs::Metadata;
use rt_format::{NoPositionalArguments, ParsedFormat};
use std::collections::HashMap;
use std::io::{Cursor, Seek};
use std::path::PathBuf;
use tmdb_api::client::Client as TmdbClient;
use tokio::io::AsyncWriteExt;
use tui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

pub mod details;
pub mod editor;
pub mod search;
pub mod table;

use crate::util::FmtStr;
use crate::views::widgets::InputState;
use crate::{AppEvent, AppMessage, AppState, ConnectionPool};
use editor::{MovieEditor, MovieEditorState};
use search::{MovieSearch, MovieSearchState};
use table::{MovieTable, MovieTableState};

#[derive(Clone, Debug, Default)]
pub struct MovieManager {
    table: MovieTable,
    search: MovieSearch,
    editor: MovieEditor,
}

#[derive(Clone, Debug, Default)]
enum InnerState {
    #[default]
    Table,
    Search(MovieSearchState),
    Editor(MovieEditorState),
}

#[derive(Clone, Debug, Default)]
pub struct MovieManagerState {
    table_state: MovieTableState,
    inner: InnerState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerEvent {
    ClearMovieList,
    MovieDiscovered((crate::nfo::Movie, usize, PathBuf)),
    MovieUpdated((crate::nfo::Movie, usize, PathBuf)),
    MovieMoved((usize, PathBuf, PathBuf)),
    SearchMovie((crate::nfo::Movie, usize, PathBuf)),
    EditMovie((crate::nfo::Movie, usize, PathBuf)),
    SearchResults(Vec<tmdb_api::movie::MovieShort>),
    OpenTable,
}
#[derive(Clone, Debug, PartialEq)]
pub enum MovieManagerMessage {
    RefreshMovies,
    SearchTitle(String),
    CreateNfo((u64, usize, PathBuf)), // tmdb_id, fs_id, movie_path
    RetrieveArtworks((crate::nfo::Movie, usize, PathBuf)),
    SaveNfo((crate::nfo::Movie, usize, PathBuf)),
    Rename((crate::nfo::Movie, usize, PathBuf)),
}

impl StatefulWidget for MovieManager {
    type State = MovieManagerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match state.inner {
            InnerState::Table => {
                StatefulWidget::render(self.table, area, buf, &mut state.table_state);
            }
            InnerState::Search(ref mut state) => {
                StatefulWidget::render(self.search, area, buf, state);
            }
            InnerState::Editor(ref mut state) => {
                StatefulWidget::render(self.editor, area, buf, state);
            }
            _ => {}
        }
    }
}

impl MovieManagerState {
    pub fn input(&mut self, app_event: AppEvent) -> bool {
        match self.inner {
            InnerState::Table => match app_event {
                AppEvent::MovieManagerEvent(MovieManagerEvent::SearchMovie((
                    movie,
                    fs_id,
                    path,
                ))) => {
                    let mut query_state = InputState::default();
                    query_state.set_value(&movie.title);
                    let new_state = MovieSearchState {
                        movie_path: path,
                        movie_fs_id: fs_id,
                        query_state,
                        ..Default::default()
                    };
                    self.inner = InnerState::Search(new_state);
                    true
                }
                AppEvent::MovieManagerEvent(MovieManagerEvent::EditMovie((movie, fs_id, path))) => {
                    let state = MovieEditorState::default().with(movie, fs_id, path);
                    self.inner = InnerState::Editor(state);
                    true
                }
                _ => self.table_state.input(app_event),
            },
            InnerState::Search(ref mut state) => {
                if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated(..)) = app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieMoved(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable) = app_event
                {
                    self.inner = InnerState::Table;
                    true
                } else {
                    state.input(app_event)
                }
            }
            InnerState::Editor(ref mut state) => {
                if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated(..)) = app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::MovieMoved(..)) =
                    app_event
                {
                    self.table_state.input(app_event)
                } else if let AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable) = app_event
                {
                    self.inner = InnerState::Table;
                    true
                } else {
                    state.input(app_event)
                }
            }
            _ => false,
        }
    }
}

impl From<MovieManagerMessage> for AppMessage {
    fn from(value: MovieManagerMessage) -> AppMessage {
        match value {
            MovieManagerMessage::RefreshMovies => {
                AppMessage::Closure(Box::new(|app_state: &mut AppState| {
                    let futures : Vec<AppEvent> = app_state
                        .libraries
                        .iter()
                        .enumerate()
                        .filter(|(_, lib)| lib.is_some())
                        .map(|(i, lib)| (i, lib.as_ref().map(|l| l.path.clone()).unwrap()))
                        .map(|(i, path)| {
                            AppEvent::ContinuationIOFuture(Box::new(move |_,_,_,conns: &ConnectionPool| Box::pin(async move {
                                let rst : Vec<Result<PathBuf>> = crate::analyze_library((conns, i), path, 4).collect().await;
                                let mut events = vec![AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList)];
                                for r in rst {
                                    match r {
                                        Ok(path) => {
                                            let placeholder_title = format!("{}", path.file_name().map(|s| s.to_string_lossy().replace(&['.', '_'], " ")).unwrap_or("Invalid file name.".into()));
                                            let movie = crate::try_open_nfo(conns.lock().await[i].as_mut().unwrap(), path.clone()).await.unwrap_or_else(|_| {
                                                crate::nfo::Movie {
                                                    title: placeholder_title,
                                                    ..Default::default()
                                                }
                                            });
                                            events.push(AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered((movie, i, path))));
                                        },
                                        Err(err) => { log::error!("An error occured while searching new titles:\n{:?}", err); },
                                    }
                                }
                                events
                            })))
                        })
                        .collect();

                    futures
                }))
            }
            MovieManagerMessage::SearchTitle(title) => AppMessage::HttpFuture(Box::new(
                |app_state: &mut AppState, _: &reqwest::Client, tmdb_client: &TmdbClient| {
                    use tmdb_api::movie::search::MovieSearch;
                    use tmdb_api::prelude::Command;
                    let ms = MovieSearch::new(title.clone())
                        .with_language(Some(
                            app_state.config.tmdb_preferences.prefered_lang.clone(),
                        ))
                        .with_region(Some(
                            app_state.config.tmdb_preferences.prefered_country.clone(),
                        ));
                    Box::pin(async move {
                        match ms.execute(&tmdb_client).await {
                            Ok(results) => {
                                vec![AppEvent::MovieManagerEvent(
                                    MovieManagerEvent::SearchResults(results.results),
                                )]
                            }
                            Err(err) => {
                                log::error!(
                                    "Movie search failed for title `{}` due to:\n{:?}",
                                    title,
                                    err
                                );
                                vec![]
                            }
                        }
                    })
                },
            )),
            MovieManagerMessage::CreateNfo((tmdb_id, fs_id, path)) => {
                AppMessage::HttpFuture(Box::new(
                    move |app_state: &mut AppState,
                          _: &reqwest::Client,
                          tmdb_client: &TmdbClient| {
                        let prefered_lang = app_state.config.tmdb_preferences.prefered_lang.clone();
                        let lib_url: Result<url::Url, ()> =
                            app_state.libraries[fs_id].as_ref().unwrap().try_into();
                        Box::pin(async move {
                            if let Ok(lib_url) = lib_url {
                                match crate::transform_as_nfo(
                                    &tmdb_client,
                                    tmdb_id,
                                    Some(prefered_lang),
                                )
                                .await
                                {
                                    Ok(mut movie_nfo) => {
                                        let lib_url = lib_url.clone();
                                        drop(tmdb_client);
                                        vec![AppEvent::ContinuationIOFuture(Box::new(
                                            move |_, _, _, conns: &ConnectionPool| {
                                                Box::pin(async move {
                                                    match async move {
                                                    let mut conns_lock = conns.lock().await;
                                                    if conns_lock[fs_id].is_none() {
                                                        return Err(anyhow!("NFO creation failed because fs_id {} does not exist anymore.", fs_id));
                                                    }
                                                    let mt = crate::get_metadata(conns_lock[fs_id].as_mut().unwrap(), lib_url, path.clone()).await?;
                                                    movie_nfo.fileinfo = Some(mt);
                                                    let nfo_string = quick_xml::se::to_string(&movie_nfo).map_err(|err| anyhow!("Failed to produce a valid NFO/XML, err:\n{:?}", err))?;
                                                    let mut helper_path = path.clone();
                                                    helper_path.set_extension("nfo");
                                                    let mut buf = Cursor::new(Vec::new());
                                                    buf.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#).await?;
                                                    buf.write_all(nfo_string.as_bytes()).await?;
                                                    let _ = buf.rewind();
                                                    let _ = conns_lock[fs_id].as_mut().unwrap().as_mut_rfs().create_file(&helper_path, &Metadata::default(), Box::new(buf))
                                                        .map_err(|err| anyhow!("Can't open the nfo file., causes:\n{:?}", err))?;
                                                    Ok(vec![
                                                        AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable),
                                                        AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((movie_nfo, fs_id, path)))
                                                    ])
                                                }.await {
                                                    Ok(ret) => ret,
                                                    Err(err) => {
                                                        log::error!("NFO Creation failed due to the following error:\n{:?}", err);
                                                        vec![]
                                                    },
                                                }
                                                })
                                            },
                                        ))]
                                    }
                                    Err(err) => {
                                        log::error!("Error occured during nfo creation (transform_as_nfo):\n{:?}", err);
                                        vec![]
                                    }
                                }
                            } else {
                                log::error!("Unable to create nfo as the current library ({}) creates an unexpected URL.", fs_id);
                                vec![]
                            }
                        })
                    },
                ))
            }
            MovieManagerMessage::RetrieveArtworks((nfo, fs_id, path)) => {
                AppMessage::IOFuture(Box::new(
                    move |_, client: &reqwest::Client, _, conns: &ConnectionPool| {
                        Box::pin(async move {
                            let mut conns_lock = conns.lock().await;
                            if conns_lock[fs_id].is_none() {
                                log::error!("Failed to retrieve artworks on fs (id: {}), as it does not exist anymore.", fs_id);
                                return vec![];
                            }
                            for th in nfo.thumb {
                                if let Some(mut aspect) = th.aspect.clone() {
                                    if aspect == "landscape" {
                                        aspect = "fanart".into()
                                    }
                                    let output = if let Some(name) =
                                        path.file_stem().map(std::ffi::OsStr::to_string_lossy)
                                    {
                                        path.with_file_name(format!("{}-{}.jpg", name, &aspect))
                                    } else {
                                        path.with_file_name(&aspect)
                                    };
                                    match crate::download_file(
                                        conns_lock[fs_id].as_mut().unwrap(),
                                        &client,
                                        output,
                                        &*format!(
                                            "https://image.tmdb.org/t/p/original{}",
                                            &th.path
                                        ),
                                    )
                                    .await
                                    {
                                        Ok(()) => {}
                                        Err(err) => {
                                            log::error!(
                                                "Failed to download {} ({}) for {}. Cause:\n{:?}",
                                                &aspect,
                                                &th.path,
                                                &nfo.title,
                                                err
                                            );
                                        }
                                    }
                                }
                            }
                            return vec![];
                        })
                    },
                ))
            }
            MovieManagerMessage::SaveNfo((nfo, fs_id, path)) => {
                AppMessage::IOFuture(Box::new(move |_, _, _, conns: &ConnectionPool| {
                    Box::pin(async move {
                        match async move {
                            let mut conns_lock = conns.lock().await;
                            if conns_lock[fs_id].is_none() {
                                return Err(anyhow!(
                                    "NFO save failed because fs_id {} does not exist anymore.",
                                    fs_id
                                ));
                            }
                            let nfo_string = quick_xml::se::to_string(&nfo).map_err(|err| {
                                anyhow!("Failed to produce a valid NFO/XML, err:\n{:?}", err)
                            })?;
                            let mut helper_path = path.clone();
                            helper_path.set_extension("nfo");
                            let mut buf = Cursor::new(Vec::new());
                            buf.write_all(
                                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
                            )
                            .await?;
                            buf.write_all(nfo_string.as_bytes()).await?;
                            let _ = buf.rewind();
                            let _ = conns_lock[fs_id]
                                .as_mut()
                                .unwrap()
                                .as_mut_rfs()
                                .create_file(&helper_path, &Metadata::default(), Box::new(buf))
                                .map_err(|err| {
                                    anyhow!("Can't open the nfo file., causes:\n{:?}", err)
                                })?;
                            Ok(vec![
                                AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable),
                                AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((
                                    nfo, fs_id, path,
                                ))),
                            ])
                        }
                        .await
                        {
                            Ok(ret) => ret,
                            Err(err) => {
                                log::error!(
                                    "NFO save failed due to the following error:\n{:?}",
                                    err
                                );
                                vec![]
                            }
                        }
                    })
                }))
            }
            MovieManagerMessage::Rename((nfo, fs_id, path)) => {
                AppMessage::IOFuture(Box::new(move |app_state, _, _, conns: &ConnectionPool| {
                    let renamer = app_state.config.renamer.clone();
                    Box::pin(async move {
                        match async move {
                            let mut conns_lock = conns.lock().await;
                            if conns_lock[fs_id].is_none() {
                                return Err(anyhow!(
                                    "Rename task failed because fs_id {} does not exist anymore.",
                                    fs_id
                                ));
                            }

                            if let Some(parent) = path.parent() {
                                let named = HashMap::from([
                                    ("title", FmtStr::new(nfo.title.as_str())),
                                    (
                                        "original_title",
                                        FmtStr::new(
                                            nfo.original_title.as_deref().unwrap_or(&nfo.title),
                                        ),
                                    ),
                                    (
                                        "release_date",
                                        FmtStr::new(
                                            nfo.premiered.as_deref().unwrap_or("XXXX-XX-XX"),
                                        ),
                                    ),
                                    (
                                        "year",
                                        FmtStr::new(
                                            nfo.premiered
                                                .as_deref()
                                                .map(|date| date[..4].to_owned())
                                                .unwrap_or("XXXX".into()),
                                        ),
                                    ),
                                    (
                                        "source",
                                        FmtStr::new(nfo.source.as_deref().unwrap_or("NONE")),
                                    ),
                                ]);
                                let dir_arg = ParsedFormat::parse(
                                    &renamer.dir_format,
                                    &NoPositionalArguments,
                                    &named,
                                )
                                .or(Err(anyhow!("dir_format is invalid!")))?;
                                let dir_name = deunicode::deunicode_with_tofu(
                                    &format!("{}", dir_arg),
                                    &renamer.dir_separator,
                                )
                                .replace(
                                    &[' ', ':', '<', '>', '?', '!', '|', '/', '\\', '*', '"'],
                                    &renamer.dir_separator,
                                );
                                let file_arg = ParsedFormat::parse(
                                    &renamer.file_format,
                                    &NoPositionalArguments,
                                    &named,
                                )
                                .or(Err(anyhow!("file_format is invalid!")))?;
                                let file_name = deunicode::deunicode_with_tofu(
                                    &format!("{}", file_arg),
                                    &renamer.file_separator,
                                )
                                .replace(
                                    &[' ', ':', '<', '>', '?', '!', '|', '/', '\\', '*', '"'],
                                    &renamer.file_separator,
                                );
                                let new_dir = parent.with_file_name(dir_name);
                                conns_lock[fs_id]
                                    .as_mut()
                                    .unwrap()
                                    .as_mut_rfs()
                                    .mov(&parent, &new_dir)
                                    .context("failed to rename the parent dir")?;
                                let entries = conns_lock[fs_id]
                                    .as_mut()
                                    .unwrap()
                                    .as_mut_rfs()
                                    .list_dir(&new_dir)
                                    .context("failed to iterate the dir entry")?;
                                let old_name = path
                                    .file_stem()
                                    .ok_or(anyhow!("Movie path does not contain a file stem."))?
                                    .to_string_lossy()
                                    .to_owned();
                                for entry in entries {
                                    if let Some(name) = entry.path.file_name() {
                                        if name.to_string_lossy().starts_with(&*old_name) {
                                            let new_name = name
                                                .to_string_lossy()
                                                .replacen(&*old_name, &file_name, 1);
                                            let new_path = entry.path().with_file_name(new_name);
                                            conns_lock[fs_id]
                                                .as_mut()
                                                .unwrap()
                                                .as_mut_rfs()
                                                .mov(&entry.path(), &new_path)
                                                .context(format!(
                                                    "failed to move {} to {}!",
                                                    entry.path.display(),
                                                    new_path.display()
                                                ))?;
                                        }
                                    }
                                }
                                let movie_name = path
                                    .file_name()
                                    .ok_or(anyhow!(
                                        "Oops, movie path does not contain a filename..."
                                    ))?
                                    .to_owned();
                                let new_path = new_dir.join(PathBuf::from(movie_name));
                                Ok(vec![AppEvent::MovieManagerEvent(
                                    MovieManagerEvent::MovieMoved((fs_id, path, new_path)),
                                )])
                            } else {
                                return Err(anyhow!(
                                    "Rename task failed because no parent exists for path {}.",
                                    path.display()
                                ));
                            }
                        }
                        .await
                        {
                            Ok(ret) => ret,
                            Err(err) => {
                                log::error!(
                                    "Rename task failed due to the following error:\n{:?}",
                                    err
                                );
                                vec![]
                            }
                        }
                    })
                }))
            }
        }
    }
}
