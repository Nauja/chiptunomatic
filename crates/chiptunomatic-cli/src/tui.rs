//! Interactive file browser: stem charts above playback panel; metadata above explorer; keys full-width below (requires `playback`).

use std::fs::File;
use std::io::{self, stdout, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context as _, Result};
use chiptunomatic::constants::{NOTE_NAMES, SAMPLE_RATE};
use chiptunomatic::{Chiptunomatic, MixerConfig, Sample, SongMetadata};
use cli_log::info;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand as _;
use ratatui::prelude::*;
use ratatui::symbols::Marker;
use ratatui::widgets::{
    Axis, Block, Borders, Chart, Dataset, FrameExt, GraphType, LineGauge, Paragraph, Wrap,
};
use ratatui_explorer::{FileExplorerBuilder, Theme};

use crate::audio::AudioPlayer;
use crate::stem_plot::{waveform_points, StemPlotBuffer};
use crate::synth::{PlaybackCancelled, PlaybackProgress, PlaybackSink, SynthCompletion};

fn stem_line_chart<'a>(label: &'a str, data: &'a [(f64, f64)], color: Color) -> Chart<'a> {
    let dataset = Dataset::default()
        .name(label)
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(color))
        .data(data);

    Chart::new(vec![dataset])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(Span::styled(
                    format!(" {label} "),
                    Style::default().fg(color),
                ))),
        )
        .x_axis(Axis::default().bounds([0.0, 1.0]))
        .y_axis(Axis::default().bounds([-1.2, 1.2]))
        .legend_position(None)
}

fn volume_bar(level: f32, height: u16) -> Paragraph<'static> {
    let level = level.clamp(0.0, 1.0);
    let filled = ((level * f32::from(height)).ceil() as u16).min(height);
    let one_third = height as f32 / 3.0;
    let lines: Vec<Line<'static>> = (0..height)
        .map(|row| {
            let from_bottom = height - 1 - row;
            if from_bottom < filled {
                let color = if (from_bottom as f32) < one_third {
                    Color::Green
                } else if (from_bottom as f32) < one_third * 2.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                Line::from(Span::styled("███", Style::default().fg(color)))
            } else {
                Line::from("   ")
            }
        })
        .collect();
    Paragraph::new(lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedButton {
    Muted(u8), // stem: 0=voice 1=square 2=triangle 3=noise 4=sfx
    Solo(u8),
}

impl FocusedButton {
    fn next(self) -> Self {
        match self {
            Self::Muted(s) => Self::Solo(s),
            Self::Solo(4) => Self::Muted(0),
            Self::Solo(s) => Self::Muted(s + 1),
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Solo(s) => Self::Muted(s),
            Self::Muted(0) => Self::Solo(4),
            Self::Muted(s) => Self::Solo(s - 1),
        }
    }
}

fn button_widget(
    label: &'static str,
    active: bool,
    focused: bool,
    active_color: Color,
) -> Paragraph<'static> {
    let style = if focused && active {
        Style::default()
            .fg(active_color)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else if active {
        Style::default()
            .fg(active_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Paragraph::new(Span::styled(label, style))
}

fn next_mode(modes: &[String], mode: &String) -> String {
    for (i, m) in modes.iter().enumerate() {
        if m == mode && i + 1 < modes.len() {
            return modes[i + 1].clone();
        }
    }
    modes[0].clone()
}

fn prev_mode(modes: &[String], mode: &String) -> String {
    for (i, m) in modes.iter().enumerate() {
        if m == mode && i > 0 {
            return modes[i - 1].clone();
        }
    }
    modes.last().cloned().unwrap_or_else(|| mode.clone())
}

enum Command {
    Play((Chiptunomatic, PathBuf, u64)),
    SetMixerConfig(MixerConfig),
}

fn spawn_audio_worker(
    cmd_rx: mpsc::Receiver<(u64, Command)>,
    done_tx: mpsc::Sender<Result<(), String>>,
    started_tx: mpsc::Sender<String>,
    ready_tx: mpsc::SyncSender<Result<(), String>>,
    live_generation: Arc<AtomicU64>,
    stem_plot: Arc<Mutex<StemPlotBuffer>>,
    playback_progress: Arc<PlaybackProgress>,
    is_paused: Arc<AtomicBool>,
) {
    let mut audio = match AudioPlayer::new(SAMPLE_RATE) {
        Ok(a) => a,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("{e:#}")));
            return;
        }
    };
    audio.sink.set_volume(1.0);
    if ready_tx.send(Ok(())).is_err() {
        return;
    }

    let mut current_instance: Option<Chiptunomatic> = None;
    let mut pending_command: Option<(u64, Command)> = None;

    'outer: loop {
        let (mut gen, mut command) = if let Some(cmd) = pending_command.take() {
            cmd
        } else {
            match cmd_rx.recv() {
                Ok(c) => c,
                Err(_) => break,
            }
        };

        // Drain the queue: keep only the latest Play, apply SetMixerConfig immediately.
        while let Ok((ng, cmd)) = cmd_rx.try_recv() {
            match cmd {
                Command::Play(_) => {
                    gen = ng;
                    command = cmd; // keep latest Play, discard earlier ones
                }
                Command::SetMixerConfig(config) => {
                    if let Some(inst) = &mut current_instance {
                        inst.mixer_mut().set_config(config);
                    }
                }
            }
        }

        match command {
            Command::SetMixerConfig(config) => {
                if let Some(inst) = &mut current_instance {
                    inst.mixer_mut().set_config(config);
                }
                continue;
            }
            Command::Play((mut instance, path, total_samples)) => {
                audio.sink.stop();
                is_paused.store(false, Ordering::SeqCst);

                if let Ok(mut plot) = stem_plot.lock() {
                    plot.clear();
                }

                let _ = started_tx.send(
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string(),
                );

                let mut sink = PlaybackSink {
                    audio: &mut audio,
                    live_generation: live_generation.clone(),
                    my_generation: gen,
                    stem_plot: stem_plot.clone(),
                    playback_progress: Arc::clone(&playback_progress),
                    is_paused: Arc::clone(&is_paused),
                };

                playback_progress.begin_track(total_samples);

                let outcome = 'synthesis: {
                    let reader = match File::open(&path) {
                        Ok(r) => r,
                        Err(e) => {
                            playback_progress.reset_idle();
                            let _ = done_tx.send(Err(e.to_string()));
                            current_instance = Some(instance);
                            continue 'outer;
                        }
                    };

                    let mut samples_reader = instance.sample_reader(reader);
                    let mut samples = [Sample::default(); 1024];

                    loop {
                        // Generate the next samples
                        match samples_reader.next_buf(&mut samples) {
                            Err(e) => {
                                playback_progress.reset_idle();
                                let _ = done_tx.send(Err(e.to_string()));
                                current_instance = Some(instance);
                                continue 'outer;
                            }
                            Ok(0) => break 'synthesis SynthCompletion::Finished,
                            Ok(n) => {
                                let chunk = &samples[0..n];
                                sink.tap_samples(chunk);
                                match sink.consume(chunk) {
                                    Ok(()) => {}
                                    Err(e) if e.is::<PlaybackCancelled>() => {
                                        break 'synthesis SynthCompletion::Interrupted;
                                    }
                                    Err(e) => {
                                        playback_progress.reset_idle();
                                        let _ = done_tx.send(Err(e.to_string()));
                                        current_instance = Some(instance);
                                        continue 'outer;
                                    }
                                }
                            }
                        }

                        // Process the commands
                        while let Ok((ng, cmd)) = cmd_rx.try_recv() {
                            match cmd {
                                Command::Play(_) => {
                                    pending_command = Some((ng, cmd));
                                    break 'synthesis SynthCompletion::Interrupted;
                                }
                                Command::SetMixerConfig(config) => {
                                    samples_reader
                                        .chiptunomatic_mut()
                                        .mixer_mut()
                                        .set_config(config);
                                }
                            }
                        }
                    }
                };

                current_instance = Some(instance);

                if live_generation.load(Ordering::SeqCst) != gen {
                    continue;
                }

                if outcome == SynthCompletion::Finished {
                    info!("Music finished");
                    playback_progress.mark_complete();
                    // Ensure remaining queued audio plays out even if paused at track end.
                    is_paused.store(false, Ordering::SeqCst);
                    audio.sink.play();
                }
            }
        }

        audio.sleep_until_end();

        if live_generation.load(Ordering::SeqCst) != gen {
            continue;
        }

        let _ = done_tx.send(Ok(()));
    }
}

fn format_playback_clock(samples: u64, sample_rate: u32) -> String {
    if sample_rate == 0 {
        return "0:00".to_string();
    }
    let secs = (samples as f64 / f64::from(sample_rate)).floor() as u64;
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

fn format_metadata(meta: &SongMetadata) -> Text<'static> {
    let root = NOTE_NAMES[meta.root_semitone as usize % 12];
    let lines = vec![
        Line::from(format!(
            "root: {}    bpm: {}    beats: {}",
            root, meta.bpm, meta.total_beats
        )),
        Line::from(format!(
            "duration: {}    data: {}",
            meta.total_duration_str, meta.data_byte_len_str
        )),
        Line::from(format!("chords: {}", meta.chord_description)),
    ];
    Text::from(lines)
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    Ok(())
}

pub fn run(mut instance: Chiptunomatic, initial_file: Option<&Path>) -> Result<()> {
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let live_generation = Arc::new(AtomicU64::new(0));
    let live_gen_worker = Arc::clone(&live_generation);

    let (cmd_tx, cmd_rx) = mpsc::channel::<(u64, Command)>();
    let modes = instance.modes_string();
    let mut mode = instance.mode_string();
    let (done_tx, done_rx) = mpsc::channel::<Result<(), String>>();
    let (started_tx, started_rx) = mpsc::channel::<String>();

    let stem_window = (SAMPLE_RATE as usize).saturating_mul(150) / 1000;
    let stem_plot = Arc::new(Mutex::new(StemPlotBuffer::new(stem_window)));
    let stem_plot_worker = Arc::clone(&stem_plot);

    let playback_progress = Arc::new(PlaybackProgress::new());
    let playback_progress_worker = Arc::clone(&playback_progress);

    let is_paused = Arc::new(AtomicBool::new(false));
    let is_paused_worker = Arc::clone(&is_paused);

    let mut mixer_config = *instance.mixer().config();

    let send_play = |instance: Chiptunomatic, path: PathBuf, total_samples: u64| {
        let gen = live_generation.fetch_add(1, Ordering::SeqCst) + 1;
        cmd_tx.send((gen, Command::Play((instance, path, total_samples))))
    };
    let send_mixer_config = |config: MixerConfig| {
        let gen = live_generation.load(Ordering::SeqCst);
        cmd_tx.send((gen, Command::SetMixerConfig(config)))
    };

    std::thread::spawn(move || {
        spawn_audio_worker(
            cmd_rx,
            done_tx,
            started_tx,
            ready_tx,
            live_gen_worker,
            stem_plot_worker,
            playback_progress_worker,
            is_paused_worker,
        )
    });

    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(msg)) => anyhow::bail!("{msg}"),
        Err(_) => anyhow::bail!("audio worker terminated before initializing"),
    }

    stdout().flush()?;
    enable_raw_mode().context("enable terminal raw mode")?;
    let mut stdout = stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("ratatui terminal")?;

    let mut file_explorer = {
        let builder = FileExplorerBuilder::default()
            .theme(Theme::default().with_title_top(|_| Line::from(" Filesystem ")));
        match initial_file {
            Some(path) => builder.working_file(path),
            None => builder,
        }
        .build()
        .map_err(anyhow::Error::from)?
    };

    let mut focused_button: Option<FocusedButton> = None;
    let mut autoplay = false;
    let mut last_sel: Option<PathBuf> = None;
    let mut meta_title: String = "Song".into();
    let mut meta_text: Text<'static> = Text::from(vec![
        Line::from(""),
        Line::from(""),
        Line::from("Select a file to preview chiptunomatic metadata here."),
    ]);
    let mut playback_hint =
        "Ready. Press Enter on a file — stops anything playing and starts that file.".to_string();
    let mut last_finished: Option<Result<(), String>> = None;
    let mut current_playing: Option<PathBuf> = None;
    let mut playing_meta: Option<Text<'static>> = None;

    if let Some(path) = initial_file {
        if path.is_file() {
            let cur = file_explorer.current();
            let (cur_path, cur_name) = (cur.path.clone(), cur.name.clone());
            let total_samples = instance
                .load_song_metadata_from_path(&cur_path)
                .map(|m| {
                    playing_meta = Some(format_metadata(&m));
                    (m.total_duration * f64::from(SAMPLE_RATE)).round() as u64
                })
                .unwrap_or(0);
            if send_play(instance.clone(), cur_path.clone(), total_samples).is_ok() {
                current_playing = Some(cur_path);
                playback_hint = format!("Starting: {cur_name}");
            }
        }
    }

    loop {
        while started_rx.try_recv().is_ok() {
            playback_hint = String::new();
        }

        while let Ok(r) = done_rx.try_recv() {
            let finished_ok = r.is_ok();
            last_finished = Some(r.clone());
            current_playing = None;
            is_paused.store(false, Ordering::SeqCst);
            playback_hint = match r {
                Ok(()) => "Playback finished.".to_string(),
                Err(e) => format!("Playback error: {e}"),
            };

            if autoplay && finished_ok {
                // Advance the explorer to the next file, skipping directories.
                let down_ev = Event::Key(KeyEvent {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                });
                for _ in 0..256 {
                    let _ = file_explorer.handle(&down_ev);
                    if file_explorer.current().is_file() {
                        break;
                    }
                }
                last_sel = None;
                let cur = file_explorer.current();
                if cur.is_file() {
                    playback_hint = format!("Autoplay: {}", cur.name);
                    let total_samples = instance
                        .load_song_metadata_from_path(&cur.path)
                        .map(|m| {
                            playing_meta = Some(format_metadata(&m));
                            (m.total_duration * f64::from(SAMPLE_RATE)).round() as u64
                        })
                        .unwrap_or(0);
                    current_playing = Some(cur.path.clone());
                    last_finished = None;
                    let _ = send_play(instance.clone(), cur.path.clone(), total_samples);
                }
            }
        }

        let current = file_explorer.current().path.clone();
        if last_sel.as_ref() != Some(&current) {
            last_sel = Some(current.clone());
            let cur_file = file_explorer.current();
            meta_title = cur_file.name.clone();
            meta_text = if cur_file.is_dir {
                Text::from(vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from("This is a directory. Open files with l / Right / Enter."),
                ])
            } else {
                match instance.song_metadata_from_path(&current) {
                    Ok(m) => format_metadata(&m),
                    Err(e) => Text::from(format!(
                        "{}\n\nCould not read metadata:\n{e:#}",
                        cur_file.name
                    )),
                }
            };
        }

        terminal.draw(|f| {
            let area = f.area();

            let enter_label =
                if current_playing.as_deref() == Some(file_explorer.current().path.as_path()) {
                    if is_paused.load(Ordering::SeqCst) {
                        " play  ·  "
                    } else {
                        " pause  ·  "
                    }
                } else {
                    " open / play  ·  "
                };

            let help_keys_text = Text::from(Line::from(vec![
                Span::styled(" q / Esc ", Style::default().fg(Color::Yellow)),
                Span::raw(" quit  ·  "),
                Span::styled(" Enter ", Style::default().fg(Color::Yellow)),
                Span::raw(enter_label),
                Span::styled(" h j k l ", Style::default().fg(Color::Yellow)),
                Span::raw(" nav  ·  "),
                Span::styled(" Ctrl+h ", Style::default().fg(Color::Yellow)),
                Span::raw(" hidden  ·  "),
                Span::styled(" m / M ", Style::default().fg(Color::Yellow)),
                Span::raw(format!(" mode [{}]  ·  ", mode)),
                Span::styled(" Tab ", Style::default().fg(Color::Yellow)),
                Span::raw(" focus M/S  ·  "),
                Span::styled(" Space ", Style::default().fg(Color::Yellow)),
                Span::raw(" toggle  ·  "),
                Span::styled(" a ", Style::default().fg(Color::Yellow)),
                Span::raw(if autoplay { " autoplay [on]" } else { " autoplay [off]" }),
            ]));
            let help_content_w = area.width.max(1);
            let help_need_lines = Paragraph::new(help_keys_text.clone())
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .line_count(help_content_w);
            let mut help_h = u16::try_from(help_need_lines).unwrap_or(u16::MAX).max(1);

            const MIN_MAIN_ROWS: u16 = 12;
            let max_help_h = area.height.saturating_sub(MIN_MAIN_ROWS).max(1);
            help_h = help_h.min(max_help_h).max(1);

            let [main_row, keys_row] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(help_h)]).areas(area);

            let [play_col, right_col] =
                Layout::horizontal([Constraint::Percentage(75), Constraint::Percentage(25)])
                    .areas(main_row);

            const MIN_EXPLORER_ROWS: u16 = 8;
            let meta_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {meta_title} metadata "));
            let meta_content_w = meta_block.inner(right_col).width.max(1);

            let meta_need_lines = Paragraph::new(meta_text.clone())
                .block(meta_block.clone())
                .wrap(Wrap { trim: true })
                .line_count(meta_content_w);
            let meta_need_u16 = u16::try_from(meta_need_lines).unwrap_or(u16::MAX).max(1);

            let max_meta_h = right_col.height.saturating_sub(MIN_EXPLORER_ROWS).max(1);
            let meta_h = meta_need_u16.min(max_meta_h.max(1)).max(1);

            let [meta_area, explorer_area] =
                Layout::vertical([Constraint::Length(meta_h), Constraint::Fill(1)])
                    .areas(right_col);

            let meta_widget = Paragraph::new(meta_text.clone())
                .block(meta_block)
                .wrap(Wrap { trim: true });

            let help = Paragraph::new(help_keys_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            let play_title_line = if let Some(name) = current_playing
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            {
                Line::from(format!(" {name} playback "))
            } else {
                match &last_finished {
                    Some(Ok(())) => Line::from(" Playback (last finished OK) "),
                    Some(Err(_)) => Line::from(" Playback (last had error) "),
                    None => Line::from(" Playback "),
                }
            };
            let play_outer = Block::default()
                .borders(Borders::ALL)
                .title(play_title_line);

            let enter_sub = if is_paused.load(Ordering::SeqCst) {
                "(Enter resumes current playback.)"
            } else {
                "(Enter stops current playback.)"
            };
            let playback_hint_inner_text = Text::from(vec![
                Line::from(playback_hint.as_str()),
                Line::from(Span::styled(
                    enter_sub,
                    Style::default().fg(Color::DarkGray),
                )),
            ]);

            let width_probe = Rect::new(0, 0, play_col.width.max(1), main_row.height.max(1));
            let hint_wrap_w = play_outer.inner(width_probe).width.max(1);
            let hint_lines = Paragraph::new(playback_hint_inner_text.clone())
                .wrap(Wrap { trim: true })
                .line_count(hint_wrap_w);
            let hint_inner_h = u16::try_from(hint_lines).unwrap_or(u16::MAX).max(1);

            let play_meta_text: Text<'static> = playing_meta.clone().unwrap_or_default();
            let play_meta_inner_h = if playing_meta.is_some() {
                let n = Paragraph::new(play_meta_text.clone())
                    .wrap(Wrap { trim: true })
                    .line_count(hint_wrap_w);
                u16::try_from(n).unwrap_or(u16::MAX).max(1)
            } else {
                0
            };

            const PROG_GAUGE_ROWS: u16 = 1;
            // Top decoration row plus bottom border (see `Block::inner`).
            const PLAYBACK_FRAME_ROWS: u16 = 2;
            let inner_content_h = play_meta_inner_h
                .saturating_add(hint_inner_h)
                .saturating_add(PROG_GAUGE_ROWS);
            let play_block_h = inner_content_h
                .saturating_add(PLAYBACK_FRAME_ROWS)
                .min(play_col.height)
                .max(PLAYBACK_FRAME_ROWS.saturating_add(1));

            let [stems_area, play_block_area] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(play_block_h)])
                    .areas(play_col);

            let play_inner = play_outer.inner(play_block_area);

            let [play_meta_area, hint_area, gauge_area] = Layout::vertical([
                Constraint::Length(play_meta_inner_h),
                Constraint::Length(hint_inner_h),
                Constraint::Length(PROG_GAUGE_ROWS),
            ])
            .areas(play_inner);

            const BAR_W: u16 = 3;
            // Each chart occupies roughly half the stems width — scale points accordingly.
            let px = (stems_area.width.saturating_sub(6) / 2) as usize;
            let px = px.max(16);

            let (vc_pts, sq_pts, tr_pts, nz_pts, sfx_pts, vc_vol, sq_vol, tr_vol, nz_vol, sfx_vol) =
                stem_plot
                    .lock()
                    .map(|g| {
                        let mv = mixer_config.effective_master_volume();
                        (
                            waveform_points(g.voice(), px),
                            waveform_points(g.square(), px),
                            waveform_points(g.triangle(), px),
                            waveform_points(g.noise(), px),
                            waveform_points(g.sfx(), px),
                            (g.voice_peak() * 3.0 * mv * mixer_config.effective_voice_volume())
                                .min(1.0),
                            (g.square_peak() * 3.0 * mv * mixer_config.effective_square_volume())
                                .min(1.0),
                            (g.triangle_peak()
                                * 3.0
                                * mv
                                * mixer_config.effective_triangle_volume())
                            .min(1.0),
                            (g.noise_peak() * 3.0 * mv * mixer_config.effective_noise_volume())
                                .min(1.0),
                            (g.sfx_peak() * 3.0 * mv * mixer_config.effective_sfx_volume())
                                .min(1.0),
                        )
                    })
                    .unwrap_or_else(|_| {
                        (
                            vec![(0.0, 0.0), (1.0, 0.0)],
                            vec![(0.0, 0.0), (1.0, 0.0)],
                            vec![(0.0, 0.0), (1.0, 0.0)],
                            vec![(0.0, 0.0), (1.0, 0.0)],
                            vec![(0.0, 0.0), (1.0, 0.0)],
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                        )
                    });

            // Layout: square occupies full top-left; top-right is voice alone or voice|sfx.
            // Bottom row: triangle (BL) | noise (BR).
            let show_sfx = instance.has_sfx();
            let [top_row, bottom_row] =
                Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                    .areas(stems_area);
            let [sq_r, top_right_r] =
                Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                    .areas(top_row);
            let (vc_r, sfx_r) = if show_sfx {
                let [v, s] =
                    Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                        .areas(top_right_r);
                (v, s)
            } else {
                (top_right_r, Rect::default())
            };
            let [tr_r, nz_r] =
                Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
                    .areas(bottom_row);

            // Returns (volume_bar, mute_btn, solo_btn) rects within a stem cell.
            // Volume bar shrinks by 2 rows to make room for M and S buttons.
            let make_stem_rects = |area: Rect| -> (Rect, Rect, Rect) {
                let inner_h = area.height.saturating_sub(2);
                let bar_h = inner_h.saturating_sub(2);
                let x = area.right().saturating_sub(1 + BAR_W);
                let w = BAR_W.min(area.width.saturating_sub(2));
                let bar = Rect::new(x, area.y + 1, w, bar_h);
                let mute = Rect::new(
                    x,
                    area.y + 1 + bar_h,
                    w,
                    inner_h.saturating_sub(bar_h).min(1),
                );
                let solo = Rect::new(
                    x,
                    area.y + 2 + bar_h,
                    w,
                    inner_h.saturating_sub(bar_h + 1).min(1),
                );
                (bar, mute, solo)
            };

            let (vc_bar_r, vc_mute_r, vc_solo_r) = make_stem_rects(vc_r);
            let (sfx_bar_r, sfx_mute_r, sfx_solo_r) = make_stem_rects(sfx_r);
            let (sq_bar_r, sq_mute_r, sq_solo_r) = make_stem_rects(sq_r);
            let (tr_bar_r, tr_mute_r, tr_solo_r) = make_stem_rects(tr_r);
            let (nz_bar_r, nz_mute_r, nz_solo_r) = make_stem_rects(nz_r);

            let total_smpl = playback_progress.total_samples();
            let elapsed_smpl = playback_progress.elapsed_samples();
            let play_ratio = playback_progress.ratio();

            let prog_label = if total_smpl == 0 {
                Line::from(" ")
            } else {
                Line::from(format!(
                    "{} / {}",
                    format_playback_clock(elapsed_smpl, SAMPLE_RATE),
                    format_playback_clock(total_smpl, SAMPLE_RATE)
                ))
            };

            let progress_gauge = LineGauge::default()
                .filled_style(Style::default().fg(Color::Cyan))
                .unfilled_style(Style::default().fg(Color::DarkGray))
                .ratio(play_ratio.clamp(0.0, 1.0))
                .label(prog_label);

            let playback_hint_txt =
                Paragraph::new(playback_hint_inner_text).wrap(Wrap { trim: true });

            f.render_widget(meta_widget, meta_area);

            f.render_widget_ref(file_explorer.widget(), explorer_area);

            f.render_widget(stem_line_chart("voice", &vc_pts, Color::LightGreen), vc_r);
            f.render_widget(volume_bar(vc_vol, vc_bar_r.height), vc_bar_r);
            f.render_widget(
                button_widget(
                    "[M]",
                    mixer_config.voice_output.muted,
                    focused_button == Some(FocusedButton::Muted(0)),
                    Color::Red,
                ),
                vc_mute_r,
            );
            f.render_widget(
                button_widget(
                    "[S]",
                    mixer_config.voice_output.solo,
                    focused_button == Some(FocusedButton::Solo(0)),
                    Color::Yellow,
                ),
                vc_solo_r,
            );

            if show_sfx {
                f.render_widget(stem_line_chart("sfx", &sfx_pts, Color::LightYellow), sfx_r);
                f.render_widget(volume_bar(sfx_vol, sfx_bar_r.height), sfx_bar_r);
                f.render_widget(
                    button_widget(
                        "[M]",
                        mixer_config.sfx_output.muted,
                        focused_button == Some(FocusedButton::Muted(4)),
                        Color::Red,
                    ),
                    sfx_mute_r,
                );
                f.render_widget(
                    button_widget(
                        "[S]",
                        mixer_config.sfx_output.solo,
                        focused_button == Some(FocusedButton::Solo(4)),
                        Color::Yellow,
                    ),
                    sfx_solo_r,
                );
            }

            f.render_widget(stem_line_chart("square", &sq_pts, Color::Cyan), sq_r);
            f.render_widget(volume_bar(sq_vol, sq_bar_r.height), sq_bar_r);
            f.render_widget(
                button_widget(
                    "[M]",
                    mixer_config.square_output.muted,
                    focused_button == Some(FocusedButton::Muted(1)),
                    Color::Red,
                ),
                sq_mute_r,
            );
            f.render_widget(
                button_widget(
                    "[S]",
                    mixer_config.square_output.solo,
                    focused_button == Some(FocusedButton::Solo(1)),
                    Color::Yellow,
                ),
                sq_solo_r,
            );

            f.render_widget(
                stem_line_chart("triangle", &tr_pts, Color::LightMagenta),
                tr_r,
            );
            f.render_widget(volume_bar(tr_vol, tr_bar_r.height), tr_bar_r);
            f.render_widget(
                button_widget(
                    "[M]",
                    mixer_config.triangle_output.muted,
                    focused_button == Some(FocusedButton::Muted(2)),
                    Color::Red,
                ),
                tr_mute_r,
            );
            f.render_widget(
                button_widget(
                    "[S]",
                    mixer_config.triangle_output.solo,
                    focused_button == Some(FocusedButton::Solo(2)),
                    Color::Yellow,
                ),
                tr_solo_r,
            );

            f.render_widget(stem_line_chart("noise", &nz_pts, Color::Yellow), nz_r);
            f.render_widget(volume_bar(nz_vol, nz_bar_r.height), nz_bar_r);
            f.render_widget(
                button_widget(
                    "[M]",
                    mixer_config.noise_output.muted,
                    focused_button == Some(FocusedButton::Muted(3)),
                    Color::Red,
                ),
                nz_mute_r,
            );
            f.render_widget(
                button_widget(
                    "[S]",
                    mixer_config.noise_output.solo,
                    focused_button == Some(FocusedButton::Solo(3)),
                    Color::Yellow,
                ),
                nz_solo_r,
            );
            if play_meta_inner_h > 0 {
                f.render_widget(
                    Paragraph::new(play_meta_text).wrap(Wrap { trim: true }),
                    play_meta_area,
                );
            }
            f.render_widget(playback_hint_txt, hint_area);
            f.render_widget(progress_gauge, gauge_area);
            f.render_widget(play_outer, play_block_area);

            f.render_widget(help, keys_row);
        })?;

        if event::poll(Duration::from_millis(1000 / 60))? {
            let ev = event::read()?;

            if let Event::Key(key) = &ev {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc if focused_button.is_none() => break,
                        KeyCode::Char('q') | KeyCode::Esc => {
                            focused_button = None;
                            continue;
                        }
                        KeyCode::Tab => {
                            focused_button =
                                Some(focused_button.map_or(FocusedButton::Muted(0), |b| b.next()));
                            continue;
                        }
                        KeyCode::BackTab => {
                            focused_button =
                                Some(focused_button.map_or(FocusedButton::Solo(4), |b| b.prev()));
                            continue;
                        }
                        KeyCode::Char(' ') if focused_button.is_some() => {
                            if let Some(btn) = focused_button {
                                match btn {
                                    FocusedButton::Muted(0) => {
                                        mixer_config.voice_output.muted =
                                            !mixer_config.voice_output.muted
                                    }
                                    FocusedButton::Muted(1) => {
                                        mixer_config.square_output.muted =
                                            !mixer_config.square_output.muted
                                    }
                                    FocusedButton::Muted(2) => {
                                        mixer_config.triangle_output.muted =
                                            !mixer_config.triangle_output.muted
                                    }
                                    FocusedButton::Muted(3) => {
                                        mixer_config.noise_output.muted =
                                            !mixer_config.noise_output.muted
                                    }
                                    FocusedButton::Muted(4) => {
                                        mixer_config.sfx_output.muted =
                                            !mixer_config.sfx_output.muted
                                    }
                                    FocusedButton::Solo(0) => {
                                        mixer_config.voice_output.solo =
                                            !mixer_config.voice_output.solo
                                    }
                                    FocusedButton::Solo(1) => {
                                        mixer_config.square_output.solo =
                                            !mixer_config.square_output.solo
                                    }
                                    FocusedButton::Solo(2) => {
                                        mixer_config.triangle_output.solo =
                                            !mixer_config.triangle_output.solo
                                    }
                                    FocusedButton::Solo(3) => {
                                        mixer_config.noise_output.solo =
                                            !mixer_config.noise_output.solo
                                    }
                                    FocusedButton::Solo(4) => {
                                        mixer_config.sfx_output.solo = !mixer_config.sfx_output.solo
                                    }
                                    _ => {}
                                }
                                send_mixer_config(mixer_config).ok();
                            }
                            continue;
                        }
                        KeyCode::Enter => {
                            let cur = file_explorer.current();
                            if cur.is_file() {
                                if current_playing.as_deref() == Some(cur.path.as_path()) {
                                    let paused = !is_paused.load(Ordering::SeqCst);
                                    is_paused.store(paused, Ordering::SeqCst);
                                    playback_hint = String::new();
                                } else {
                                    is_paused.store(false, Ordering::SeqCst);
                                    let verb = if current_playing.is_some() {
                                        "Stopping current playback; starting"
                                    } else {
                                        "Starting"
                                    };
                                    playback_hint = format!("{verb}: {}", cur.name);
                                    let total_samples = instance
                                        .load_song_metadata_from_path(&cur.path)
                                        .map(|m| {
                                            playing_meta = Some(format_metadata(&m));
                                            (m.total_duration * f64::from(SAMPLE_RATE)).round()
                                                as u64
                                        })
                                        .unwrap_or(0);
                                    current_playing = Some(cur.path.clone());
                                    last_finished = None;
                                    let _ = send_play(
                                        instance.clone(),
                                        cur.path.clone(),
                                        total_samples,
                                    );
                                }
                                continue;
                            }
                        }
                        KeyCode::Char('m') => {
                            mode = next_mode(&modes, &mode);
                            instance.set_mode(&mode)?;
                            last_sel = None;
                            if let Some(path) = current_playing.clone() {
                                is_paused.store(false, Ordering::SeqCst);
                                playback_hint = format!("Mode: {} — restarting", mode);
                                let total_samples = instance
                                    .load_song_metadata_from_path(&path)
                                    .map(|m| {
                                        playing_meta = Some(format_metadata(&m));
                                        (m.total_duration * f64::from(SAMPLE_RATE)).round() as u64
                                    })
                                    .unwrap_or(0);
                                last_finished = None;
                                let _ = send_play(instance.clone(), path.clone(), total_samples);
                            } else {
                                playback_hint = format!("Mode: {}", mode);
                            }
                            continue;
                        }
                        KeyCode::Char('a') if focused_button.is_none() => {
                            autoplay = !autoplay;
                            playback_hint = if autoplay {
                                "Autoplay enabled.".to_string()
                            } else {
                                "Autoplay disabled.".to_string()
                            };
                            continue;
                        }
                        KeyCode::Char('M') => {
                            mode = prev_mode(&modes, &mode);
                            instance.set_mode(&mode)?;
                            last_sel = None;
                            if let Some(path) = current_playing.clone() {
                                is_paused.store(false, Ordering::SeqCst);
                                playback_hint = format!("Mode: {} — restarting", mode);
                                let total_samples = instance
                                    .load_song_metadata_from_path(&path)
                                    .map(|m| {
                                        playing_meta = Some(format_metadata(&m));
                                        (m.total_duration * f64::from(SAMPLE_RATE)).round() as u64
                                    })
                                    .unwrap_or(0);
                                last_finished = None;
                                let _ = send_play(instance.clone(), path.clone(), total_samples);
                            } else {
                                playback_hint = format!("Mode: {}", mode);
                            }
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            if let Err(e) = file_explorer.handle(&ev) {
                playback_hint = format!("Explorer input error: {e}");
            }
        }
    }

    cleanup_terminal(&mut terminal).ok();

    Ok(())
}
