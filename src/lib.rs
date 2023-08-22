use anyhow::{anyhow, bail, Result};
use async_recursion::async_recursion;
use async_stream::try_stream;
use core::convert::AsRef;
use futures_core::stream::Stream;
use futures_util::stream::StreamExt;
use remotefs::fs::Metadata;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::{io::Cursor, io::Seek};
use tmdb_api::client::Client as TmdbClient;
use tmdb_api::{
    movie::credits::MovieCredits, movie::details::MovieDetails, movie::images::MovieImages,
    prelude::*,
};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

pub mod config;
pub mod library;
pub mod localfs;
pub mod multifs;
pub mod nfo;
pub mod util;
pub mod views;

use multifs::{MultiFs, OwnedCursor};
pub use views::{AppEvent, AppMessage, AppState};

const VIDEO_EXTENSIONS: &'static [&'static str] = &[
    "mp4", "mov", "flv", "mkv", "webm", "m4v", "avi", "iso", "wmw", "mpg",
];
pub static MESSAGE_SENDER: OnceLock<UnboundedSender<AppMessage>> = OnceLock::new();

pub type ConnectionPool = tokio::sync::Mutex<Vec<Option<crate::multifs::MultiFs>>>;

pub async fn download_file<'a, U>(
    lfs: &mut MultiFs,
    client: &reqwest::Client,
    output: PathBuf,
    url: U,
) -> Result<()>
where
    U: Into<&'a str> + Clone,
{
    let rsp = client
        .get(url.clone().into())
        .send()
        .await
        .map_err(|err| anyhow!("Failed to request {}, causes:\n{:?}", url.into(), err))?;

    let data = rsp.bytes().await.map_err(|err| {
        anyhow!(
            "Failed to read incoming data for {}, causes:\n{:?}",
            output.display(),
            err
        )
    })?;

    let buf = Cursor::new(Vec::from(data.as_ref()));

    let _ = lfs
        .as_mut_rfs()
        .create_file(&output, &Metadata::default(), Box::new(buf))
        .map_err(|err| {
            anyhow!(
                "Failed to create(or open) file {}, causes:\n{:?}",
                output.display(),
                err
            )
        })?;

    log::info!("Sucessfully downloaded file {}.", output.display());
    Ok(())
}

pub async fn try_open_nfo(lfs: &mut MultiFs, mut path: PathBuf) -> Result<nfo::Movie> {
    let mut oc = OwnedCursor::new();
    let cursor = Box::new(oc.clone());
    if path.set_extension("nfo") {
        if let Ok(_) = lfs.as_mut_rfs().open_file(&path, cursor.clone()) {
            let buf_cursor = Box::new(std::io::BufReader::new(oc.clone()));
            let _ = oc.rewind();
            let movie: nfo::Movie = quick_xml::de::from_reader(buf_cursor).map_err(|err| {
                anyhow!(
                    "Failed to read nfo at {}, causes:\n{:?}",
                    path.display(),
                    err
                )
            })?;
            return Ok(movie);
        }
    }
    path.push("movie.nfo");
    if let Ok(_) = lfs.as_mut_rfs().open_file(&path, cursor) {
        let buf_cursor = Box::new(std::io::BufReader::new(oc.clone()));
        let _ = oc.rewind();
        let movie: nfo::Movie = quick_xml::de::from_reader(buf_cursor).map_err(|err| {
            anyhow!(
                "Failed to read nfo at {}, causes:\n{:?}",
                path.display(),
                err
            )
        })?;
        return Ok(movie);
    }
    Err(anyhow!("No nfo available."))
}

pub async fn get_metadata(
    lfs: &mut MultiFs,
    base_url: Url,
    path: PathBuf,
) -> Result<nfo::FileInfo> {
    use metadata::stream::StreamMetadata;

    let meta =
        multifs::open_multifs_media(lfs.as_mut_rfs(), base_url, path.clone()).map_err(|err| {
            anyhow!(
                "Unable to get metadata for file {}, causes:\n{:?}",
                path.display(),
                err
            )
        })?;
    let mut vtracks = Vec::new();
    let mut atracks = Vec::new();
    let mut stracks = Vec::new();
    for track in meta._streams_metadata {
        match track {
            StreamMetadata::VideoMetadata(vt) => {
                let dar = (vt._display_aspect_ratio.0 as f32) / (vt._display_aspect_ratio.1 as f32);
                let vi = nfo::VideoTrack {
                    codec: vt._codec.name().to_string(),
                    aspect: Some(format!("{:.2}", dar)),
                    width: Some(vt.width.into()),
                    height: Some(vt.height.into()),
                    duration_in_seconds: meta._duration.map(|dur| dur as u64),
                    language: None,
                    hdr_type: None,
                };
                vtracks.push(vi);
            }
            StreamMetadata::AudioMetadata(at) => {
                let ai = nfo::AudioTrack {
                    codec: at._codec.name().to_string(),
                    language: at.language.clone(),
                    channels: Some(at._channel_layout.channels() as u64),
                };
                atracks.push(ai);
            }
            StreamMetadata::SubtitleMetadata(st) => {
                let si = nfo::SubtitleTrack {
                    codec: Some(st._codec.name().to_string()),
                    language: st.language.clone(),
                };
                stracks.push(si);
            }
            _ => {}
        }
    }
    let sd = nfo::StreamDetails {
        video: vtracks,
        audio: atracks,
        subtitle: stracks,
    };
    Ok(nfo::FileInfo { streamdetails: sd })
}

pub async fn transform_as_nfo(
    client: &TmdbClient,
    tmdb_id: u64,
    lang: Option<String>,
) -> Result<nfo::Movie> {
    let mdr = MovieDetails::new(tmdb_id).with_language(lang.clone());
    let md = mdr.execute(&client).await.map_err(|err| {
        anyhow!(
            "Failed to get movie details (id: {}), causes:\n{:?}",
            tmdb_id,
            err
        )
    })?;
    let mcr = MovieCredits::new(tmdb_id);
    let mc = mcr.execute(&client).await.map_err(|err| {
        anyhow!(
            "Failed to get movie credits (id: {}), causes:\n{:?}",
            tmdb_id,
            err
        )
    })?;
    let mir = MovieImages::new(tmdb_id).with_language(lang);
    let mi = mir.execute(&client).await.map_err(|err| {
        anyhow!(
            "Failed to get movie image (id: {}), causes:\n{:?}",
            tmdb_id,
            err
        )
    })?;
    let mira = MovieImages::new(tmdb_id);
    let mia = mira.execute(&client).await.map_err(|err| {
        anyhow!(
            "Failed to get movie image (id: {}), causes:\n{:?}",
            tmdb_id,
            err
        )
    })?;

    let mut actors = Vec::new();
    let mut directors = Vec::new();
    let mut producers = Vec::new();
    for p in mc.cast {
        let thumb = if let Some(path) = p.person.profile_path {
            Some(nfo::Thumb {
                aspect: None,
                path: format!("https://image.tmdb.org/t/p/original{}", path),
            })
        } else {
            None
        };
        let actor = nfo::Actor {
            name: p.person.name.clone(),
            tmdbid: Some(p.person.id),
            role: vec![p.character.clone()],
            order: Some(p.order),
            thumb,
        };
        actors.push(actor);
    }

    for p in mc.crew {
        let person = nfo::CrewPerson {
            name: p.person.name.clone(),
            tmdbid: Some(p.person.id),
            thumb: p.person.profile_path.map(|url| nfo::Thumb {
                aspect: None,
                path: format!("https://image.tmdb.org/t/p/original{}", url),
            }),
        };
        if &p.job == "Director" {
            directors.push(person);
        } else if &p.job == "Producer" {
            producers.push(person);
        }
    }

    let mut thumb = Vec::new();
    if let Some(bd) = mi.backdrops.first().or(mia.backdrops.first()) {
        let art = nfo::Thumb {
            aspect: Some("landscape".into()),
            path: format!("https://image.tmdb.org/t/p/original{}", &bd.file_path),
        };
        thumb.push(art);
    }

    if let Some(poster) = mi.posters.first().or(mia.posters.first()) {
        let art = nfo::Thumb {
            aspect: Some("poster".into()),
            path: format!("https://image.tmdb.org/t/p/original{}", &poster.file_path),
        };
        thumb.push(art);
    }

    let tmdb_uid = nfo::UniqueId {
        default: true,
        id_type: "tmdb".into(),
        value: tmdb_id.to_string(),
    };

    let movie = nfo::Movie {
        title: md.inner.title.clone(),
        original_title: Some(md.inner.original_title.clone()),
        plot: Some(md.inner.overview),
        uniqueid: vec![tmdb_uid],
        genre: md.genres.into_iter().map(|g| g.name.clone()).collect(),
        tag: vec![],
        country: md
            .production_countries
            .into_iter()
            .map(|pc| pc.name.clone())
            .collect(),
        credits: vec![],
        director: directors,
        producer: producers,
        premiered: md
            .inner
            .release_date
            .map(|rd| rd.format("%Y-%m-%d").to_string()),
        studio: md
            .production_companies
            .into_iter()
            .map(|pc| pc.name.clone())
            .collect(),
        actor: actors,
        thumb,
        runtime: md.runtime,
        tagline: md.tagline.clone(),
        source: None,
        fileinfo: None,
    };

    Ok(movie)
}

pub fn analyze_library<'a>(
    conn: (&'a ConnectionPool, usize),
    path: PathBuf,
    depth: usize,
) -> LibraryStream<'a> {
    LibraryStream::new(conn, path, depth)
}

pub struct LibraryStream<'a> {
    conn: (&'a ConnectionPool, usize),
    depth: usize,
    sub_streams: Vec<Pin<Box<LibraryStream<'a>>>>,
    found_path: Vec<PathBuf>,
    search_future: Option<Pin<Box<dyn Future<Output = Result<Vec<(PathBuf, bool)>>> + 'a>>>,
}

impl<'a> LibraryStream<'a> {
    pub fn new(
        conn: (&'a ConnectionPool, usize),
        path: PathBuf,
        depth: usize,
    ) -> LibraryStream<'a> {
        LibraryStream {
            conn,
            depth,
            search_future: Some(Box::pin(LibraryStream::search(conn.clone(), path, depth))),
            sub_streams: Vec::new(),
            found_path: Vec::new(),
        }
    }

    async fn search(
        conn: (&'a ConnectionPool, usize),
        path: PathBuf,
        depth: usize,
    ) -> Result<Vec<(PathBuf, bool)>> {
        let dir;
        {
            let mut conn_lock = conn.0.lock().await;
            let lfs = (match conn_lock.get_mut(conn.1) {
                Some(c) => match c {
                    Some(ref mut lfs) => Ok::<&mut MultiFs, anyhow::Error>(lfs),
                    None => bail!(
                        "Searching a path on an unexistant library {} (editted or deleted).",
                        conn.1
                    ),
                },
                None => bail!(
                    "Searching a path on an unexistant library {} (never existed).",
                    conn.1
                ),
            })?;
            let no_media = path.join("./.nomedia");
            if lfs.as_mut_rfs().exists(&no_media).map_err(|err| {
                anyhow!(
                    "Failed to open directory {}, causes:\n{:?}",
                    &no_media.display(),
                    err
                )
            })? {
                log::info!("Ignoring entry {} (.nomedia).", path.display());
                return Ok(vec![]);
            }
            dir = lfs.as_mut_rfs().list_dir(&path).map_err(|err| {
                anyhow!(
                    "Failed to open directory {}, causes:\n{:?}",
                    &path.display(),
                    err
                )
            })?;
        }
        let mut video_paths = Vec::new();
        for entry in dir {
            if entry.metadata().file_type.is_file() {
                if entry.path().extension().is_some()
                    && VIDEO_EXTENSIONS
                        .contains(&entry.path().extension().unwrap().to_string_lossy().as_ref())
                {
                    log::debug!("Found {}!", entry.path().display());
                    video_paths.push((entry.path().to_owned(), false));
                } else {
                    log::debug!(
                        "Ignored {} (not a video container)!",
                        entry.path().display()
                    );
                }
            } else if entry.is_dir() {
                if entry.path().ends_with(".") || entry.path().ends_with("..") {
                    continue;
                }
                if depth > 0 {
                    video_paths.push((entry.path().to_owned(), true));
                }
            } else {
                log::debug!("Ignoring entry {} (symlink).", entry.path().display());
            }
        }
        Ok(video_paths)
    }
}

impl<'a> Stream for LibraryStream<'a> {
    type Item = Result<PathBuf>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let ls = self.as_mut().get_mut();
        if let Some(fut) = ls.search_future.as_mut() {
            match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(rst) => match rst {
                    Ok(paths) => {
                        for (path, is_dir) in paths {
                            if !is_dir {
                                ls.found_path.push(path);
                            } else {
                                let depth = ls.depth;
                                let conn = ls.conn;
                                ls.sub_streams.push(Box::pin(LibraryStream::new(
                                    conn,
                                    path,
                                    depth - 1,
                                )));
                            }
                        }
                        ls.search_future = None;
                        self.poll_next(cx)
                    }
                    Err(err) => Poll::Ready(Some(Err(err))),
                },
            }
        } else {
            if let Some(path) = ls.found_path.pop() {
                Poll::Ready(Some(Ok(path)))
            } else {
                for i in 0..ls.sub_streams.len() {
                    let sub = ls.sub_streams.get_mut(i).unwrap();
                    match sub.as_mut().poll_next(cx) {
                        Poll::Pending => {}
                        Poll::Ready(Some(Ok(path))) => {
                            ls.found_path.push(path);
                        }
                        Poll::Ready(None) => {
                            ls.sub_streams.swap_remove(i);
                        }
                        Poll::Ready(Some(Err(err))) => {
                            return Poll::Ready(Some(Err(err)));
                        }
                    }
                }
                if let Some(path) = ls.found_path.pop() {
                    Poll::Ready(Some(Ok(path)))
                } else {
                    Poll::Ready(None)
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.search_future.is_none() {
            if self.depth == 0 {
                (self.found_path.len(), Some(self.found_path.len()))
            } else {
                let upper = self
                    .sub_streams
                    .iter()
                    .map(|sub| sub.size_hint().1)
                    .chain(std::iter::once(Some(self.found_path.len())))
                    .sum();
                (self.found_path.len(), upper)
            }
        } else {
            (0, None)
        }
    }
}
