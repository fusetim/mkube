use anyhow::{anyhow, Result};
use futures_core::stream::Stream;
use futures_util::stream::{select_all, StreamExt};
use remotefs::fs::Metadata;
use std::io::{Cursor, Seek};
use std::path::PathBuf;
use tmdb_api::client::Client as TmdbClient;
use tokio::io::AsyncWriteExt;
use tui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

pub mod details;
pub mod editor;
pub mod search;
pub mod table;

use crate::views::widgets::InputState;
use crate::{nfo, AppEvent, AppMessage, AppState, ConnectionPool};
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

async fn unroll_movies<T: 'static + Unpin + Sync + Send + Stream<Item = Vec<Result<AppEvent>>>>(
    mut stream: T,
) -> Vec<AppEvent> {
    if let Some(rst) = stream.next().await {
        let mut events: Vec<AppEvent> = rst
            .into_iter()
            .filter_map(|evt| match evt {
                Ok(e) => Some(e),
                Err(err) => {
                    log::error!("Error occured while refreshing movie:\n{:?}", err);
                    None
                }
            })
            .collect();
        events.push(AppEvent::ContinuationIOFuture(Box::new(|_, _, _, _| {
            Box::pin(unroll_movies(stream))
        })));
        events
    } else {
        vec![]
    }
}

impl From<MovieManagerMessage> for AppMessage {
    fn from(value: MovieManagerMessage) -> AppMessage {
        match value {
            MovieManagerMessage::RefreshMovies => AppMessage::IOFuture(Box::new(
                |app_state: &mut AppState, _, _, conns: &ConnectionPool| {
                    let streams = app_state
                        .libraries
                        .iter()
                        .enumerate()
                        .filter(|(i, lib)| lib.is_some())
                        .map(|(i, lib)| (i, lib.map(|l| l.path.clone()).unwrap()))
                        .map(|(i, path)| (i, crate::analyze_library((conns, i), path, 4)))
                        .map(|(i, stream)| {
                            stream.map(|path| match path {
                                Ok(path) => {
                                    let placeholder_title = format!(
                                        "{}",
                                        path.file_name()
                                            .map(|s| s.to_string_lossy().replace(&['.', '_'], " "))
                                            .unwrap_or("Invalid file name.".into())
                                    );
                                    let movie = crate::nfo::Movie {
                                        title: placeholder_title,
                                        ..Default::default()
                                    };
                                    Ok(AppEvent::MovieManagerEvent(
                                        MovieManagerEvent::MovieDiscovered((movie, i, path)),
                                    ))
                                }
                                Err(err) => Err(err),
                            })
                        });
                    let stream = select_all(streams).ready_chunks(20);
                    Box::pin(async {
                        vec![
                            AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList),
                            AppEvent::ContinuationIOFuture(Box::new(|_, _, _, _| {
                                Box::pin(unroll_movies(stream))
                            })),
                        ]
                    })
                },
            )),
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
        }
    }
}
