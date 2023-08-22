use anyhow::{anyhow, Result};
use futures_util::{FutureExt, StreamExt};
use std::collections::HashMap;
use std::io;
use tmdb_api::client::Client as TmdbClient;
use tokio::sync::{mpsc::unbounded_channel, Mutex};
use tokio::task::JoinSet;
use tokio::time::{self, Duration};

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
use mkube::views::AppEvent;
use mkube::{multifs, views, ConnectionPool};

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

    match tokio::task::LocalSet::new()
        .run_until(run(&mut terminal))
        .await
    {
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
    let tmdb_client: &'static TmdbClient = Box::leak(Box::new(TmdbClient::new(
        "74a673b58f22dd90b8ac750b62e00b0b".into(),
    )));
    let http_client: &'static reqwest::Client = Box::leak(Box::new(reqwest::Client::new()));
    let conns: &'static ConnectionPool = Box::leak(Box::new(Mutex::new(Vec::new())));
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
        .set(sender.clone())
        .map_err(|err| anyhow!("Failed to init MESSAGE_SENDER, causes:\n{:?}", err))?;
    let cfg: mkube::config::Configuration = confy::load(APP_NAME, CONFIG_NAME)?;
    let app = views::App {
        settings_page: views::settings::SettingsPage::new(),
        movie_manager: Default::default(),
    };
    let mut state = views::AppState {
        config: cfg,
        ..Default::default()
    };
    let mut event_reader = EventStream::new();
    let mut pending_futures: JoinSet<Vec<AppEvent>> = JoinSet::new();
    let tick = time::interval(Duration::from_millis(1000 / 15));
    tokio::pin!(tick);

    // Load libraries from config.
    #[cfg(feature = "secrets")]
    {
        keyring
            .unlock()
            .await
            .map_err(|err| anyhow!("Failed to unlock keyring, causes:\n{:?}", err))?;
        for lib in state.config.libraries.iter_mut().flatten() {
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
    {
        let mut conns_lock = conns.lock().await;
        for lib in state.config.libraries.iter().flatten() {
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
                conns_lock.push(Some(conn));
                if cfg!(feature = "secrets") {
                    state.libraries.push(Some(lib_));
                } else {
                    state.libraries.push(Some(lib_));
                }
            }
        }
    }

    loop {
        let event = event_reader.next().fuse();

        tokio::select! {
            _ = tick.tick() => {
                terminal.draw(|f| {
                    let size = f.size();
                    f.render_stateful_widget(app.clone(), size, &mut state);
                })?;
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
                    use mkube::{ views::movie_manager::{MovieManagerMessage}};
                    match msg {
                        AppMessage::Closure(closure) => {
                            for evt in closure(&mut state) {
                                state.register_event(evt);
                            }
                        }
                        AppMessage::Future(builder) => {
                            let _ = pending_futures.spawn(builder(&mut state));
                        },
                        AppMessage::AppFuture(builder) => {
                            for evt in builder(&mut state).await {
                                state.register_event(evt);
                            }
                        },
                        AppMessage::IOFuture(builder) => {
                            let _ = pending_futures.spawn_local(builder(&mut state, &http_client, &tmdb_client, &conns));
                        },
                        AppMessage::HttpFuture(builder) => {
                            let _ = pending_futures.spawn(builder(&mut state, &http_client, &tmdb_client));
                        },
                        AppMessage::TriggerEvent(evt) => {
                            state.register_event(evt);
                        },
                        AppMessage::SettingsMessage(SettingsMessage::EditExisting(lib)) => {
                            if let Some((ind, _)) = state.libraries.iter().enumerate().filter(|(_, l)| l.is_some() && l.as_ref().unwrap() == &lib).next() {
                                let l = state.libraries[ind].clone().unwrap();
                                // Safety: Delete conn first, otherwise the app might panic if a future try to access this library.
                                conns.lock().await[ind] = None;
                                state.libraries[ind] = None;
                                state.config.libraries[ind] = None;
                                state.register_event(AppEvent::SettingsEvent(SettingsEvent::EditExisting(l)));
                            } else {
                                log::error!("Invalid library editing, message ignored.");
                            }
                        },
                        AppMessage::SettingsMessage(SettingsMessage::SaveLibrary(lib)) => {
                            if let Ok(mut conn) = MultiFs::try_from(&lib) {
                                if !conn.as_mut_rfs().is_connected() { let _ = conn.as_mut_rfs().connect(); }
                                conns.lock().await.push(Some(conn));
                                state.libraries.push(Some(lib.clone()));
                                #[cfg(feature = "secrets")]
                                {
                                    state.config.libraries.push(Some(ConfigLibrary::from_with_keyring(lib, &keyring).await));
                                }
                                #[cfg(not(feature = "secrets"))]
                                {
                                    state.config.libraries.push(Some(lib.into()));
                                }
                                if let Err(err) = confy::store(APP_NAME, CONFIG_NAME, &state.config) {
                                    log::error!("Failed to save configuration, causes:\n{:?}", err);
                                }
                            }
                            state.register_event(AppEvent::SettingsEvent(SettingsEvent::OpenMenu(state.libraries.iter().flatten().cloned().collect())));
                        },
                        AppMessage::SettingsMessage(SettingsMessage::OpenMenu)
                        | AppMessage::SettingsMessage(SettingsMessage::TestLibrary(_))
                        | AppMessage::MovieManagerMessage(MovieManagerMessage::RefreshMovies)
                        | AppMessage::MovieManagerMessage(MovieManagerMessage::SearchTitle(_))
                        | AppMessage::MovieManagerMessage(MovieManagerMessage::CreateNfo(_))
                        | AppMessage::MovieManagerMessage(MovieManagerMessage::SaveNfo(_))
                        | AppMessage::MovieManagerMessage(MovieManagerMessage::RetrieveArtworks(_)) => {
                            panic!("Deprecated");
                        },
                        AppMessage::Close => {
                            break;
                        },
                        _ => { unimplemented!(); }
                    }
                } else {
                    break;
                }
            },
            task = pending_futures.join_next(), if !pending_futures.is_empty() => match task {
                Some(task) => match task {
                    Ok(evt) => for e in evt {
                        state.register_event(e);
                    },
                    Err(err) => {
                        log::error!("pending_futures has returned an error:\n{:?}", err);
                    },
                },
                None => {
                    log::warn!("pending_futures is empty but still has been processed! This is a BUG!");
                },
            }
        }
    }

    if let Err(err) = confy::store(APP_NAME, CONFIG_NAME, &state.config) {
        log::error!("Failed to save configuration, causes:\n{:?}", err);
    }

    Ok(())
}
