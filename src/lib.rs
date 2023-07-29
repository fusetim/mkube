use std::{io, io::Seek, thread, time::Duration, io::Cursor};
use std::path::{PathBuf, Path};
use std::str::FromStr;
use std::sync::OnceLock;
use core::convert::AsRef;
use anyhow::{Result, anyhow};
use tmdb_api::{
  prelude::*,
  common::PaginatedResult,
  common::credits::Cast,
  movie::search::MovieSearch,
  movie::details::MovieDetails,
  movie::credits::{MovieCredits, MovieCreditsResult},
  movie::images::{MovieImages, MovieImagesResult},
  movie::{MovieShort, MovieBase},
};
use tmdb_api::client::Client as TmdbClient;
use remotefs::fs::{RemoteFs, Metadata};
use tokio::fs::{File, read_dir};
use tokio::io::{AsyncWriteExt,AsyncReadExt};
use tokio::sync::mpsc::{UnboundedSender};
use async_recursion::async_recursion;
use url::Url;
use remotefs_ftp::client::FtpFs;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
    Frame,
    terminal,
    terminal::{Terminal},
};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,};
use crossterm::execute;
use crossterm::event::{EnableMouseCapture, DisableMouseCapture, KeyEvent};

pub mod views;
pub mod nfo;
pub mod localfs;
pub mod multifs;
pub mod util;
pub mod library;

use multifs::{MultiFs, OwnedCursor};
pub use views::{AppMessage, AppEvent, AppState};

const VIDEO_EXTENSIONS: &'static [&'static str] = &["mp4", "mov", "flv", "mkv", "webm", "m4v", "avi", "iso", "wmw", "mpg"];
pub static MESSAGE_SENDER: OnceLock<UnboundedSender<AppMessage>> = OnceLock::new();

async fn download_file<'a, U>(lfs: &mut MultiFs, client: &reqwest::Client, output: PathBuf, url: U) -> Result<()>
where U: Into<&'a str> + Clone {
    let mut rsp = client.get(url.clone().into())
        .send().await
        .map_err(|err| anyhow!("Failed to request {}, causes:\n{:?}", url.into(), err))?;

    let data = rsp.bytes().await.map_err(|err| anyhow!("Failed to read incoming data for {}, causes:\n{:?}", output.display(), err))?;

    let buf = Cursor::new(Vec::from(data.as_ref()));

    let _ = lfs.as_mut_rfs().create_file(&output, &Metadata::default(), Box::new(buf))
        .map_err(|err| anyhow!("Failed to create(or open) file {}, causes:\n{:?}", output.display(), err))?;

    println!("Sucessfully downloaded file {}.", output.display());
    Ok(())
}

#[async_recursion(?Send)]
pub async fn analyze_library(lfs: &mut MultiFs, path: PathBuf, depth: usize) -> Result<Vec<PathBuf>> {
    //let mut dir = read_dir(&path).await.map_err(|err| anyhow!("Failed to open directory {}, causes:\n{:?}", &path.display(), err))?;
    let mut dir = lfs.as_mut_rfs().list_dir(&path).map_err(|err| anyhow!("Failed to open directory {}, causes:\n{:?}", &path.display(), err))?;
    let mut video_paths = Vec::new();
    for entry in dir {
        if entry.metadata().file_type.is_file() {
            if entry.path().extension().is_some() && VIDEO_EXTENSIONS.contains(&entry.path().extension().unwrap().to_string_lossy().as_ref()) {
                //println!("Found {}!", entry.path().display());
                video_paths.push(entry.path().to_owned());
            } else {
                //println!("Ignored {} (not a video container)!", entry.path().display());
            }
        } else if entry.is_dir() {
            let no_media = entry.path().join("./.nomedia");
            if lfs.as_mut_rfs().exists(&no_media).map_err(|err| anyhow!("Failed to open directory {}, causes:\n{:?}", &no_media.display(), err))? {
                //println!("Ignoring entry {} (.nomedia).", entry.path().display());
            } else if depth > 0 { 
                let sub = analyze_library(lfs, entry.path().to_owned(), depth - 1).await?;
                video_paths.extend(sub);
            }
        } else {
            //println!("Ignoring entry {} (symlink).", entry.path().display());
        }
    }
    Ok(video_paths)
}

pub async fn try_open_nfo(lfs: &mut MultiFs, mut path: PathBuf) -> Result<nfo::Movie> {
    let mut oc = OwnedCursor::new();
    let cursor = Box::new(oc.clone());
    if path.set_extension("nfo") {
        if let Ok(_) = lfs.as_mut_rfs().open_file(&path, cursor.clone()) {
            let buf_cursor = Box::new(std::io::BufReader::new(oc.clone()));
            let _ = oc.rewind();
            let movie : nfo::Movie = quick_xml::de::from_reader(buf_cursor).map_err(|err| anyhow!("Failed to read nfo at {}, causes:\n{:?}", path.display(), err))?;
            return Ok(movie);
        }
    }
    path.push("movie.nfo");
    if let Ok(_) = lfs.as_mut_rfs().open_file(&path, cursor) {
        let buf_cursor = Box::new(std::io::BufReader::new(oc.clone()));
        let _ = oc.rewind();
        let movie : nfo::Movie = quick_xml::de::from_reader(buf_cursor).map_err(|err| anyhow!("Failed to read nfo at {}, causes:\n{:?}", path.display(), err))?;
        return Ok(movie);
    }
    Err(anyhow!("No nfo available."))
}

pub async fn get_metadata(lfs: &mut MultiFs, base_url: Url, path: PathBuf) -> Result<nfo::FileInfo> {
    use metadata::media_file::MediaFileMetadata;
    use metadata::stream::StreamMetadata;

    let meta = multifs::open_multifs_media(lfs.as_mut_rfs(), base_url, path.clone())
        .map_err(|err| anyhow!("Unable to get metadata for file {}, causes:\n{:?}", path.display(), err))?;
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
            },
            StreamMetadata::AudioMetadata(at) => {
                let ai = nfo::AudioTrack { 
                    codec: at._codec.name().to_string(),
                    language: at.language.clone(),
                    channels: Some(at._channel_layout.channels() as u64),
                };
                atracks.push(ai);
            },
            StreamMetadata::SubtitleMetadata(st) => {
                let si = nfo::SubtitleTrack { 
                    codec: Some(st._codec.name().to_string()),
                    language: st.language.clone(),
                };
                stracks.push(si);
            },
            _ => {},
        }
    }
    let sd = nfo::StreamDetails {
        video: vtracks,
        audio: atracks,
        subtitle: stracks,
    };
    Ok(nfo::FileInfo {
        streamdetails: sd,
    })
}

pub async fn transform_as_nfo(client: &TmdbClient, tmdb_id: u64, lang: Option<String>) -> Result<nfo::Movie> {
    let mdr = MovieDetails::new(tmdb_id).with_language(lang.clone());
    let md = mdr.execute(&client).await
        .map_err(|err| anyhow!("Failed to get movie details (id: {}), causes:\n{:?}", tmdb_id, err))?;
    let mcr = MovieCredits::new(tmdb_id);
    let mc = mcr.execute(&client).await
        .map_err(|err| anyhow!("Failed to get movie credits (id: {}), causes:\n{:?}", tmdb_id, err))?;
    let mir = MovieImages::new(tmdb_id).with_language(lang);
    let mi = mir.execute(&client).await
        .map_err(|err| anyhow!("Failed to get movie image (id: {}), causes:\n{:?}", tmdb_id, err))?;
    let mira = MovieImages::new(tmdb_id);
    let mia = mira.execute(&client).await
            .map_err(|err| anyhow!("Failed to get movie image (id: {}), causes:\n{:?}", tmdb_id, err))?;
    
    let mut actors = Vec::new();
    let mut directors = Vec::new();
    let mut producers = Vec::new();
    for p in mc.cast {
        let thumb = if let Some(path) = p.person.profile_path {
            Some(nfo::Thumb {
                aspect: None,
                path: format!("https://image.tmdb.org/t/p/original{}", path),
            })
        } else { None };
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
            thumb: p.person.profile_path.map(|url| nfo::Thumb{ aspect: None, path: format!("https://image.tmdb.org/t/p/original{}", url)}),
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
        country: md.production_countries.into_iter().map(|pc| pc.name.clone()).collect(),
        credits: vec![],
        director: directors,
        producer: producers,
        premiered: md.inner.release_date.map(|rd| rd.format("%Y-%m-%d").to_string()),
        studio: md.production_companies.into_iter().map(|pc| pc.name.clone()).collect(),
        actor: actors,
        thumb,
        runtime: md.runtime,
        tagline: md.tagline.clone(),
        source: None,
        fileinfo: None,
    };

    Ok(movie)
}

