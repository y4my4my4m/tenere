use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::{env, io, path::PathBuf};
use tenere::app::{App, AppResult};
use tenere::config::{Config, TTSConfig};
use tenere::event::{Event, EventHandler, TTSEvent};
use tenere::formatter::Formatter;
use tenere::handler::handle_key_events;
use tenere::llm::{LLMAnswer, LLMRole, LLM}; // Add LLM import
use tenere::tui::Tui;
use tenere::tts;

use tenere::llm::LLMModel;

use std::sync::Arc;
use tokio::sync::Mutex;

use clap::{crate_description, crate_version, Arg, Command};

use ratatui::backend::Backend; // Add this import

#[tokio::main]
async fn main() -> AppResult<()> {
    let matches = Command::new("tenere")
        .about(crate_description!())
        .version(crate_version!())
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to custom config file")
                .value_name("FILE"),
        )
        .get_matches();

    let config_path = matches.get_one::<String>("config").map(PathBuf::from);
    let config = Arc::new(Config::load(config_path));

    let (formatter_config, formatter_assets) = Formatter::init();
    let formatter = Formatter::new(&formatter_config, &formatter_assets);

    let mut app = App::new(config.clone(), &formatter);

    let llm = Arc::new(Mutex::new(
        LLMModel::init(&config.llm, config.clone()).await,
    ));

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    // create data directory if not exists
    app.history
        .check_data_directory_exists(tui.events.sender.clone());

    // load potential history data from archive files
    app.history.load_history(tui.events.sender.clone());

    // Make sure to clean up TTS processes on exit
    let result = run_app(&mut app, llm, &mut tui, &formatter, &config).await;
    
    // Clean up TTS processes before exiting
    tts::kill_all_tts_processes();
    
    tui.exit()?;
    result
}

async fn run_app<B: Backend>(
    app: &mut App<'_>, 
    llm: Arc<Mutex<Box<dyn LLM + 'static>>>, 
    tui: &mut Tui<B>,
    formatter: &Formatter<'_>,
    config: &Arc<Config>
) -> AppResult<()> {
    while app.running {
        tui.draw(app)?;
        match tui.events.next().await? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => {
                handle_key_events(key_event, app, llm.clone(), tui.events.sender.clone())
                    .await?;
            }
            Event::Mouse(_) => {}
            Event::Resize(_, _) => {}
            Event::LLMEvent(LLMAnswer::Answer(answer)) => {
                app.chat
                    .handle_answer(LLMAnswer::Answer(answer.clone()), formatter);
                
                // We don't want to trigger TTS for every tiny chunk
                // Only send longer message portions to avoid choppy audio
                if answer.len() > 80 && answer.contains('.') {
                    tui.events.sender.send(Event::TTSEvent(TTSEvent::PlayText { 
                        text: answer,
                        voice: None 
                    }))?;
                }
            }
            Event::LLMEvent(LLMAnswer::EndAnswer) => {
                {
                    let mut llm = llm.lock().await;
                    llm.append_chat_msg(app.chat.answer.plain_answer.clone(), LLMRole::ASSISTANT);
                    
                    // Play the full response with TTS when it completes,
                    // using the default voice from config if set.
                    let final_answer = app.chat.answer.plain_answer.clone();
                    if !final_answer.is_empty() {
                        tui.events.sender.send(Event::TTSEvent(TTSEvent::PlayText {
                            text: final_answer,
                            voice: config.tts.default_voice.clone(), // Optional default voice
                        }))?;
                    }
                }
                app.chat.handle_answer(LLMAnswer::EndAnswer, formatter);
                app.terminate_response_signal
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }
            Event::LLMEvent(LLMAnswer::StartAnswer) => {
                app.spinner.active = false;
                app.chat.handle_answer(LLMAnswer::StartAnswer, formatter);
            }

            Event::Notification(notification) => {
                app.notifications.push(notification);
            }
            Event::TTSEvent(tts_event) => {
                handle_tts_event(tts_event, &config.tts).await;
            }
        }
    }

    Ok(())
}

async fn handle_tts_event(event: TTSEvent, tts_config: &TTSConfig) {
    match event {
        TTSEvent::PlayText { text, voice: _ } => {
            // Clone what we need to move into the background task
            let tts_config = tts_config.clone();
            let text = text.clone();
            
            // Spawn a background task for TTS playback to avoid blocking the UI
            tokio::spawn(async move {
                if let Err(e) = tts::play_tts(&text, &tts_config).await {
                    eprintln!("TTS error: {}", e);
                }
            });
            
            // Return immediately so the main application can continue handling input
        },
        TTSEvent::Complete => {
            // TTS playback completed
        },
        TTSEvent::Error(err) => {
            eprintln!("TTS error: {}", err);
        }
    }
}
