use anyhow::{anyhow, Result};
use futures_util::{FutureExt, StreamExt};
use remotefs::fs::Metadata;
use std::io;
use tmdb_api::client::Client as TmdbClient;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::{self, Duration};

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use tui::{backend::CrosstermBackend, terminal::Terminal};

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
            println!("Exit success.");
        }
        Err(err) => {
            eprintln!("Exit failed, caused:\n{:?}", err);
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

async fn run<B>(terminal: &mut Terminal<B>) -> Result<()>
where
    B: tui::backend::Backend,
{
    let (sender, mut receiver) = unbounded_channel();
    let tmdb_client = TmdbClient::new("74a673b58f22dd90b8ac750b62e00b0b".into());
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
    for lib in &cfg.libraries {
        if let Ok(mut conn) = MultiFs::try_from(lib) {
            if !conn.as_mut_rfs().is_connected() {
                let _ = conn.as_mut_rfs().connect();
            }
            state.conns.push(conn);
            state.libraries.push(lib.clone());
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
                                let _ = state.conns.swap_remove(ind);
                                state.register_event(AppEvent::SettingsEvent(SettingsEvent::EditExisting(l)));
                            }
                        },
                        AppMessage::SettingsMessage(SettingsMessage::SaveLibrary(lib)) => {
                            if let Ok(mut conn) = MultiFs::try_from(&lib) {
                                if !conn.as_mut_rfs().is_connected() { let _ = conn.as_mut_rfs().connect(); }
                                state.conns.push(conn);
                                state.libraries.push(lib.clone());
                                cfg.libraries.push(lib);
                                let _ = confy::store(APP_NAME, CONFIG_NAME, &cfg);
                            }
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(state.libraries.clone())));
                        },
                        AppMessage::SettingsMessage(SettingsMessage::TestLibrary(lib)) => {
                            let rst = if let Ok(mut conn) = MultiFs::try_from(&lib) {
                                let _ = conn.as_mut_rfs().connect();
                                (conn.as_mut_rfs().is_connected(), conn.as_mut_rfs().exists(&lib.path.as_path()).unwrap_or(false))
                            } else {
                                (false, false)
                            };
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::ConnTestResult(rst)));
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::RefreshMovies) => {
                            state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::ClearMovieList));
                            for i in 0..state.conns.len() {
                                let _ = state.conns[i].as_mut_rfs().connect();
                                if state.conns[i].as_mut_rfs().is_connected() {
                                    if let Ok(paths) = mkube::analyze_library(&mut state.conns[i], state.libraries[i].path.clone(), 2).await {
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
                                    }
                                }
                            }
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::SearchTitle(title)) => {
                            use tmdb_api::movie::search::MovieSearch;
                            use tmdb_api::prelude::Command;
                            let ms = MovieSearch::new(title)
                                .with_language(Some(cfg.tmdb_preferences.prefered_lang.clone()))
                                .with_region(Some(cfg.tmdb_preferences.prefered_country.clone()));
                            if let Ok(results) = ms.execute(&tmdb_client).await {
                                state.register_event(AppEvent::MovieManagerEvent(MovieManagerEvent::SearchResults(results.results)));
                            } else {
                                // TODO
                            }
                        },
                        AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo((id, fs_id, mut path))) => {
                            use std::io::Cursor;
                            use std::io::Seek;
                            let mut movie_nfo = mkube::transform_as_nfo(&tmdb_client, id, Some(cfg.tmdb_preferences.prefered_lang.clone())).await?;
                            let mt = mkube::get_metadata(&mut state.conns[fs_id], (&state.libraries[fs_id]).try_into().expect("Cannot get a baseURL from library."), path.clone()).await?;
                            movie_nfo.fileinfo = Some(mt);
                            let nfo_string = quick_xml::se::to_string(&movie_nfo).expect("Failed to produce a valid nfo file.");

                            path.set_extension("nfo");
                            let mut buf = Cursor::new(Vec::new());
                            buf.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#).await?;
                            buf.write_all(nfo_string.as_bytes()).await?;
                            let _ = buf.rewind();
                            let _ = state.conns[fs_id].as_mut_rfs().create_file(&path, &Metadata::default(), Box::new(buf))
                                .map_err(|err| anyhow!("Can't open the nfo file., causes:\n{:?}", err))?;
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
