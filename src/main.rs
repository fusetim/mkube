use anyhow::{anyhow, Result};
use futures_util::{FutureExt, StreamExt};
use remotefs::fs::Metadata;
use std::collections::HashMap;
use std::io;
use tmdb_api::client::Client as TmdbClient;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::{self, Duration};
use url::Url;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use tui::{backend::CrosstermBackend, terminal::Terminal, widgets::Paragraph};

#[cfg(feature = "secrets")]
use oo7::Keyring;

use mkube::config::{ConfigLibrary, Credentials};
use mkube::multifs;
use mkube::views;

use multifs::MultiFs;

const APP_NAME: &'static str = "mkube";
const CONFIG_NAME: Option<&'static str> = Some("config");

#[tokio::main]
async fn main() -> Result<()> {
    init_logger().await;
    log::info!("Hello!");

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    log::info!("Terminal successfully prepared!");

    match run(&mut terminal).await {
        Ok(()) => {
            log::info!("Exit success.");
        }
        Err(err) => {
            log::error!("Exit failed, caused:\n{:?}", err);
        }
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    log::info!("Terminal successfully restored!");

    Ok(())
}

async fn init_logger() {
    use structured_logger::{async_json::new_writer, Builder};
    let log_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("mkube.log")
        .await
        .unwrap();
    Builder::new()
        .with_default_writer(new_writer(log_file))
        .init();
}

#[cfg(feature = "secrets")]
async fn init_keyring() -> Result<Keyring> {
    use anyhow::bail;
    use oo7::portal::Secret;
    use rand::thread_rng;
    use rand::RngCore;
    use std::sync::Arc;
    use std::time::Duration as StdDuration;
    use tokio::time::timeout;
    match oo7::portal::Keyring::load_default().await {
        Ok(keyring) => Ok(Keyring::File(Arc::new(keyring))),
        Err(err) => {
            log::error!("Failed to init keyring using Secret Portal, falling back to Secret DBus API + Keyring File. Cause:\n{:?}", err);
            let kr = Keyring::new().await?;
            match timeout(StdDuration::from_secs(60), kr.unlock()).await {
                Ok(rst) => match rst {
                    Ok(()) => {}
                    Err(err) => bail!("Failed to unlock keyring. Cause:\n{:?}", err),
                },
                Err(elapsed) => {
                    bail!("Timeout, failed to unlock keyring. Cause:\n{:?}", elapsed);
                }
            }
            let attrs = HashMap::from([("app", APP_NAME), ("secret", "keyring_key")]);
            let key: Vec<u8> = if let Some(key_) = kr
                .search_items(attrs.clone())
                .await
                .map_err(|err| anyhow!("Failed to find the keyring_key, err:\n{:?}", err))?
                .get(0)
            {
                key_.secret().await?.to_vec()
            } else {
                let mut rng = thread_rng();
                let mut key_ = [0; 1024];
                RngCore::fill_bytes(&mut rng, &mut key_[..]);
                kr.create_item("MKube Keyring key", attrs, &key_, true)
                    .await?;
                key_.to_vec()
            };
            //kr.lock().await?;
            let keyring = oo7::portal::Keyring::load(
                confy::get_configuration_file_path(APP_NAME, None)?.join("../keyring.key"),
                Secret::from(key),
            )
            .await?;
            Ok(Keyring::File(Arc::new(keyring)))
        }
    }
}

async fn run<B>(terminal: &mut Terminal<B>) -> Result<()>
where
    B: tui::backend::Backend,
{
    terminal.draw(|f| {
        let size = f.size();
        f.render_widget(
            Paragraph::new("Initializing... (You might need to unlock your system KeyWallet.)"),
            size,
        );
    })?;
    let (sender, mut receiver) = unbounded_channel();
    let tmdb_client = TmdbClient::new("74a673b58f22dd90b8ac750b62e00b0b".into());
    let keyring;
    #[cfg(feature = "secrets")]
    {
        keyring = init_keyring().await?;
    }
    #[cfg(not(feature = "secrets"))]
    {
        keyring = ()
    }
    mkube::MESSAGE_SENDER
        .set(sender)
        .map_err(|err| anyhow!("Failed to init MESSAGE_SENDER, causes:\n{:?}", err))?;
    let mut cfg: mkube::config::Configuration = confy::load(APP_NAME, CONFIG_NAME)?;
    let app = views::App {
        settings_page: views::settings::SettingsPage::new(),
        movie_manager: Default::default(),
    };
    let mut state = views::AppState::default();
    let mut event_reader = EventStream::new();
    let tick = time::interval(Duration::from_millis(1000 / 15));
    tokio::pin!(tick);

    // Load libraries from config.
    #[cfg(feature = "secrets")]
    {
        keyring
            .unlock()
            .await
            .map_err(|err| anyhow!("Failed to unlock keyring, causes:\n{:?}", err))?;
        for lib in cfg.libraries.iter_mut() {
            if let Credentials::ToKeyring(c) = lib.password.clone() {
                let path = lib.path.display().to_string();
                let attributes = HashMap::from([
                    ("fs_type", lib.fs_type.to_scheme()),
                    ("host", lib.host.as_deref().unwrap_or("")),
                    ("username", lib.username.as_deref().unwrap_or("")),
                    ("path", &path),
                ]);
                match keyring.create_item(&lib.name, attributes, &c, true).await {
                    Ok(()) => lib.password = Credentials::Keyring,
                    Err(err) => {
                        log::error!("Failed to save credentials to keyring, the credentials will be saved as clear text temporary. Cause:\n{:?}", err);
                    }
                }
            }
        }
        keyring
            .lock()
            .await
            .map_err(|err| anyhow!("Failed to lock keyring, causes:\n{:?}", err))?;
    }
    for lib in &cfg.libraries {
        let lib_;
        #[cfg(feature = "secrets")]
        {
            lib_ = ConfigLibrary::try_into_with_keyring(lib.clone(), &keyring).await?;
        }

        #[cfg(not(feature = "secrets"))]
        {
            lib_ = ConfigLibrary::into(lib.clone());
        }

        if let Ok(mut conn) = MultiFs::try_from(&lib_) {
            if !conn.as_mut_rfs().is_connected() {
                let _ = conn.as_mut_rfs().connect();
            }
            state.conns.push(conn);
            if cfg!(feature = "secrets") {
                state.libraries.push(lib_);
            } else {
                state.libraries.push(lib_);
            }
        }
    }

    loop {
        let event = event_reader.next().fuse();

        tokio::select! {
            _ = tick.tick() => {
                //println!("tick!");
                state.tick();
                terminal.draw(|f| {
                    let size = f.size();
                    f.render_stateful_widget(app.clone(), size, &mut state);
                })?;
                state.clear_events();
            }
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if let Event::Key(kev) = event {
                            if kev.code == KeyCode::Char('c') && kev.modifiers == KeyModifiers::CONTROL {
                                break;
                            }
                            state.register_event(mkube::AppEvent::KeyEvent(kev));
                        }

                        if event == Event::Key(KeyCode::Esc.into()) {
                            break;
                        }
                    }
                    Some(Err(e)) => println!("Error: {:?}\r", e),
                    None => break,
                }
            }
            msg = receiver.recv() => {
                if let Some(msg) = msg {
                    use mkube::{AppMessage, AppEvent, views::settings::{SettingsMessage, SettingsEvent}};
                    use mkube::{ views::movie_manager::{MovieManagerEvent, MovieManagerMessage}};
                    match msg {
                        AppMessage::Future(builder) => {
                            if let Some(app_event) = builder(&mut state).await {
                                state.register_event(app_event);
                            }
                        },
                        AppMessage::TriggerEvent(evt) => {
                            state.register_event(evt);
                        },
                        AppMessage::SettingsMessage(SettingsMessage::OpenMenu) => {
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(state.libraries.clone())));
                        },
                        AppMessage::SettingsMessage(SettingsMessage::EditExisting(lib)) => {
                            if let Some((ind, _)) = state.libraries.iter().enumerate().filter(|(_, l)| &&lib == l).next() {
                                let l = state.libraries.swap_remove(ind);
                                let _ = cfg.libraries.swap_remove(ind);
                                let _ = state.conns.swap_remove(ind);
                                state.register_event(AppEvent::SettingsEvent(SettingsEvent::EditExisting(l)));
                            } else {
                                log::error!("Invalid library editing, message ignored.");
                            }
                        },
                        AppMessage::SettingsMessage(SettingsMessage::SaveLibrary(lib)) => {
                            if let Ok(mut conn) = MultiFs::try_from(&lib) {
                                if !conn.as_mut_rfs().is_connected() { let _ = conn.as_mut_rfs().connect(); }
                                state.conns.push(conn);
                                state.libraries.push(lib.clone());
                                #[cfg(feature = "secrets")]
                                {
                                    cfg.libraries.push(ConfigLibrary::from_with_keyring(lib, &keyring).await);
                                }
                                #[cfg(not(feature = "secrets"))]
                                {
                                    cfg.libraries.push(lib.into());
                                }
                                if let Err(err) = confy::store(APP_NAME, CONFIG_NAME, &cfg) {
                                    log::error!("Failed to save configuration, causes:\n{:?}", err);
                                }
                            }
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(state.libraries.clone())));
                        },
                        AppMessage::SettingsMessage(SettingsMessage::TestLibrary(lib)) => {
                            let rst = match MultiFs::try_from(&lib) {
                                Ok(mut conn) => {
                                    let _ = conn.as_mut_rfs().connect();
                                    (conn.as_mut_rfs().is_connected(), conn.as_mut_rfs().exists(&lib.path.as_path()).unwrap_or(false))
                                },
                                Err(err) => {
                                    log::warn!("Connection to library `{}` failed due to:\n{:?}", Url::try_from(&lib).as_ref().map(Url::as_ref).unwrap_or("N/A"), err);
                                    (false, false)
                                },
                            };
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::ConnTestResult(rst)));
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::RefreshMovies) => {
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList));
                            for i in 0..state.conns.len() {
                                let _ = state.conns[i].as_mut_rfs().connect();
                                if state.conns[i].as_mut_rfs().is_connected() {
                                    match mkube::analyze_library(&mut state.conns[i], state.libraries[i].path.clone(), 2).await {
                                        Ok(paths) => {
                                            for path in paths {
                                                let placeholder_title = format!("{}", path.file_name().map(|s| s.to_string_lossy().to_owned()).unwrap_or("Invalid file name.".into()));
                                                let movie = mkube::try_open_nfo(&mut state.conns[i], path.clone()).await.unwrap_or_else(|_| {
                                                    mkube::nfo::Movie {
                                                        title: placeholder_title,
                                                        ..Default::default()
                                                    }
                                                });
                                                state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::MovieDiscovered((movie, i, path))));
                                            }
                                        },
                                        Err(err) => {
                                            log::error!("Failed to analyze library `{}` due to:\n{:?}", Url::try_from(& state.libraries[i]).as_ref().map(Url::as_ref).unwrap_or("N/A"), err);
                                        }
                                    }
                                }
                            }
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::SearchTitle(title)) => {
                            use tmdb_api::movie::search::MovieSearch;
                            use tmdb_api::prelude::Command;
                            let ms = MovieSearch::new(title.clone())
                                .with_language(Some(cfg.tmdb_preferences.prefered_lang.clone()))
                                .with_region(Some(cfg.tmdb_preferences.prefered_country.clone()));
                            match ms.execute(&tmdb_client).await {
                                Ok(results) => { state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::SearchResults(results.results))); },
                                Err(err) => { log::error!("Movie search failed for title `{}` due to:\n{:?}", title, err); },
                            }
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::CreateNfo((id, fs_id, mut path))) => {
                            use std::io::Cursor;
                            use std::io::Seek;
                            let mut movie_nfo = mkube::transform_as_nfo(&tmdb_client, id, Some(cfg.tmdb_preferences.prefered_lang.clone())).await?;
                            let mt = mkube::get_metadata(&mut state.conns[fs_id], (&state.libraries[fs_id]).try_into().expect("Cannot get a baseURL from library."), path.clone()).await?;
                            movie_nfo.fileinfo = Some(mt);
                            let nfo_string = quick_xml::se::to_string(&movie_nfo).expect("Failed to produce a valid nfo file.");
                            let movie_path= path.clone();

                            path.set_extension("nfo");
                            let mut buf = Cursor::new(Vec::new());
                            buf.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#).await?;
                            buf.write_all(nfo_string.as_bytes()).await?;
                            let _ = buf.rewind();
                            let _ = state.conns[fs_id].as_mut_rfs().create_file(&path, &Metadata::default(), Box::new(buf))
                                .map_err(|err| anyhow!("Can't open the nfo file., causes:\n{:?}", err))?;
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable));
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((movie_nfo, fs_id, movie_path))));
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((movie_nfo, fs_id, mut path))) => {
                            use std::io::Cursor;
                            use std::io::Seek;
                            let nfo_string = quick_xml::se::to_string(&movie_nfo).expect("Failed to produce a valid nfo file.");
                            let movie_path= path.clone();

                            path.set_extension("nfo");
                            let mut buf = Cursor::new(Vec::new());
                            buf.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#).await?;
                            buf.write_all(nfo_string.as_bytes()).await?;
                            let _ = buf.rewind();
                            let _ = state.conns[fs_id].as_mut_rfs().create_file(&path, &Metadata::default(), Box::new(buf))
                                .map_err(|err| anyhow!("Can't open the nfo file., causes:\n{:?}", err))?;
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::OpenTable));
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::MovieUpdated((movie_nfo, fs_id, movie_path))));
                        },
                        AppMessage::Close => {
                            break;
                        },
                        _ => { unimplemented!(); }
                    }
                } else {
                    break;
                }
            }
        }
    }

    if let Err(err) = confy::store(APP_NAME, CONFIG_NAME, &cfg) {
        log::error!("Failed to save configuration, causes:\n{:?}", err);
    }

    Ok(())
}

/*
#[tokio::main]
async fn main() -> Result<()> {

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let app = views::App {};
    let mut state = views::AppState{events: vec![]};

    terminal.draw(|f| {
        let size = f.size();
        f.render_stateful_widget(app, size, &mut state);
    })?;

    thread::sleep(Duration::from_millis(5000));

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let tmdb_client = TmdbClient::new("74a673b58f22dd90b8ac750b62e00b0b".into());
    let client = reqwest::Client::new();


    /*let mut ftp = FtpFs::new("server", 21)
        .username("user")
        .password("pass");
    // connect
    assert!(ftp.connect().is_ok());
    assert!(ftp.change_dir(Path::new("/Enregistrements/TVRip/")).is_ok());
    */

    let mut lfs = MultiFs::Local(localfs::LocalFs::new("/home/fusetim/Developing/IntelliJ/mkube/test_dir/".into()));
    //let mut lfs = MultiFs::Ftp(ftp);
    let base_url = Url::parse("file:///home/fusetim/Developing/IntelliJ/mkube/test_dir/").unwrap();
    //let base_url = Url::parse("ftp://user:pass@server:21/Enregistrements/TVRip/").unwrap();
    /*let ms = MovieSearch::new("I, Robot".into());
    let results = ms.execute(&tmdb_client).await.expect("FF");
    for mss in &results.results {
        let mb = &mss.inner;
        let rd = if let Some(rd) = mb.release_date {
            rd.format("%Y").to_string()
        } else { "N/A".into() };
        let pp = if let Some(path) = &mb.poster_path {
            format!("https://image.tmdb.org/t/p/original{}", path)
        } else { "N/A".into() };
        println!("Found {} ({}) - poster at {}", mb.title, rd, pp);
    }

    let first = &results.results[0];
    let nfo = transform_as_nfo(&tmdb_client, first.inner.id).await?;
    let nfo_string = quick_xml::se::to_string(&nfo).expect("FFFF");
    println!("{}", nfo_string);*/

    let movies = analyze_library(&mut lfs, PathBuf::from_str(".").unwrap(), 1).await?;
    let mut nfo_tasks = Vec::new();
    for path in movies {
        nfo_tasks.push((path.clone(), try_open_nfo(&mut lfs, path).await));
    }

    for task in nfo_tasks {
        match task {
            (path, Ok(movie)) => {
                println!("[NFO] Found {} at {}.", movie.title, path.display());
                /*let mt = get_metadata(path.clone()).await?;
                dbg!(mt);*/
            },
            (mut path, Err(err)) => {
                println!("[NFO] {} do not have a NFO, causes:\n{:?}", path.display(), err);
                let name = path.file_stem()
                    .ok_or(anyhow!("No filename"))
                    .map(|title| title.to_string_lossy())
                    .map(|title| {
                        let fragments : Vec<&str> = title.split('.').collect();
                        fragments[0].replace("-", " ").replace("_", " ")
                    })?;
                let ms = MovieSearch::new(name.clone()).with_language(Some("fr".to_owned())).with_region(Some("FR".to_owned()));
                let results = ms.execute(&tmdb_client).await.expect("Looking title on TMDB");
                if results.results.len() <= 0 {
                    println!("[NFO][SKIP] No result found for {} on TMDB", name);
                    continue;
                }
                let mut movie_nfo = transform_as_nfo(&tmdb_client, results.results[0].inner.id, Some("fr".to_owned())).await?;
                let mt = get_metadata(&mut lfs, base_url.clone(), path.clone()).await?;
                movie_nfo.fileinfo = Some(mt);
                let nfo_string = quick_xml::se::to_string(&movie_nfo).expect("FFFF");

                path.set_extension("nfo");
                let mut buf = Cursor::new(Vec::new());
                buf.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#).await?;
                buf.write_all(nfo_string.as_bytes()).await?;
                let _ = buf.rewind();
                let _ = lfs.as_mut_rfs().create_file(&path, &Metadata::default(), Box::new(buf))
                    .map_err(|err| anyhow!("Can't open the nfo file., causes:\n{:?}", err))?;

                let fs = path.file_stem().map(|s| s.to_string_lossy().to_owned()).unwrap_or("movie".into());
                for th in movie_nfo.thumb {
                    if let Some(aspect) = th.aspect {
                        let output = path.with_file_name(format!("{}-{}.jpg", &fs, aspect));
                        download_file(&mut lfs, &client, output, &*format!("https://image.tmdb.org/t/p/original{}", &th.path)).await?;
                    }
                }
            },
        }
    }

    Ok(())
}*/
