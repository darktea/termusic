/**
 * MIT License
 *
 * termusic - Copyright (C) 2021 Larry Hao
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#[cfg(feature = "discord")]
use crate::discord::Rpc;
#[cfg(feature = "mpris")]
mod mpris;
mod update;
mod view;
mod youtube_options;
use crate::sqlite::{DataBase, SearchCriteria};
#[cfg(feature = "cover")]
use crate::ueberzug::UeInstance;
use crate::{
    config::Settings,
    track::Track,
    ui::{Application, Id, Msg},
};

use crate::config::{Keys, StyleColorSymbol};
use crate::player::{GeneralPlayer, Loop, PlayerTrait};
use crate::podcast::{db::Database as DBPod, Podcast, PodcastFeed, Threadpool};
use crate::songtag::SongTag;
use crate::sqlite::TrackForDB;
use crate::track::MediaType;
use crate::ui::SearchLyricState;
use crate::utils::{get_app_config_path, DownloadTracker};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tui_realm_treeview::Tree;
use tuirealm::event::NoUserEvent;
use tuirealm::terminal::TerminalBridge;
pub use youtube_options::YoutubeOptions;

#[derive(PartialEq, Eq)]
pub enum TermusicLayout {
    TreeView,
    DataBase,
    Podcast,
}

#[derive(PartialEq, Clone, Eq)]
pub enum ConfigEditorLayout {
    General,
    Color,
    Key1,
    Key2,
}

pub struct Model {
    /// Indicates that the application must quit
    pub quit: bool,
    /// Tells whether to redraw interface
    pub redraw: bool,
    last_redraw: Instant,
    pub app: Application<Id, Msg, NoUserEvent>,
    /// Used to draw to terminal
    pub terminal: TerminalBridge,
    pub path: PathBuf,
    pub tree: Tree,
    pub config: Settings,
    pub player: GeneralPlayer,
    pub yanked_node_id: Option<String>,
    // pub current_song: Option<Track>,
    pub tageditor_song: Option<Track>,
    pub time_pos: i64,
    pub lyric_line: String,
    pub lyric_last: String,
    youtube_options: YoutubeOptions,
    #[cfg(feature = "cover")]
    pub ueberzug_instance: UeInstance,
    pub songtag_options: Vec<SongTag>,
    pub sender_songtag: Sender<SearchLyricState>,
    pub receiver_songtag: Receiver<SearchLyricState>,
    pub viuer_supported: ViuerSupported,
    pub ce_themes: Vec<String>,
    pub ce_style_color_symbol: StyleColorSymbol,
    pub ke_key_config: Keys,
    #[cfg(feature = "mpris")]
    pub mpris: mpris::Mpris,
    #[cfg(feature = "discord")]
    pub discord: Rpc,
    pub db: DataBase,
    pub db_criteria: SearchCriteria,
    pub db_search_results: Vec<String>,
    pub db_search_tracks: Vec<TrackForDB>,
    pub layout: TermusicLayout,
    pub config_layout: ConfigEditorLayout,
    pub config_changed: bool,
    pub download_tracker: DownloadTracker,
    pub podcasts: Vec<Podcast>,
    pub podcasts_index: usize,
    pub db_podcast: DBPod,
    pub threadpool: Threadpool,
    pub tx_to_main: Sender<Msg>,
    pub rx_to_main: Receiver<Msg>,
    pub podcast_search_vec: Option<Vec<PodcastFeed>>,
}

#[derive(Debug)]
pub enum ViuerSupported {
    Kitty,
    ITerm,
    // Sixel,
    NotSupported,
}

impl Model {
    pub fn new(config: &Settings) -> Self {
        let path = Self::get_full_path_from_config(config);
        let tree = Tree::new(Self::library_dir_tree(&path, config.max_depth_cli));

        let (tx3, rx3): (Sender<SearchLyricState>, Receiver<SearchLyricState>) = mpsc::channel();

        let mut viuer_supported = ViuerSupported::NotSupported;
        if viuer::KittySupport::None != viuer::get_kitty_support() {
            viuer_supported = ViuerSupported::Kitty;
        // } else if viuer::is_sixel_supported() {
        // viuer_supported = ViuerSupported::Sixel;
        } else if viuer::is_iterm_supported() {
            viuer_supported = ViuerSupported::ITerm;
        }
        let db = DataBase::new(config);
        let db_criteria = SearchCriteria::Artist;
        let app = Self::init_app(&tree, config);
        let terminal = TerminalBridge::new().expect("Could not initialize terminal");
        let player = GeneralPlayer::new(config);
        // let viuer_supported =
        //     viuer::KittySupport::None != viuer::get_kitty_support() || viuer::is_iterm_supported();

        #[cfg(feature = "cover")]
        let ueberzug_instance = UeInstance::default();

        #[cfg(feature = "discord")]
        let discord = Rpc::default();

        let db_path = get_app_config_path().expect("failed to get podcast db path.");

        let db_podcast = DBPod::connect(&db_path).expect("error connecting to podcast db.");

        let podcasts = db_podcast
            .get_podcasts()
            .expect("failed to get podcasts from db.");
        let threadpool = Threadpool::new(config.podcast_simultanious_download);
        let (tx_to_main, rx_to_main) = mpsc::channel();

        Self {
            app,
            quit: false,
            redraw: true,
            last_redraw: Instant::now(),
            tree,
            path,
            terminal,
            config: config.clone(),
            player,
            yanked_node_id: None,
            // current_song: None,
            tageditor_song: None,
            time_pos: 0,
            lyric_line: String::new(),
            lyric_last: String::new(),
            youtube_options: YoutubeOptions::new(),
            #[cfg(feature = "cover")]
            ueberzug_instance,
            songtag_options: vec![],
            sender_songtag: tx3,
            receiver_songtag: rx3,
            viuer_supported,
            ce_themes: vec![],
            ce_style_color_symbol: StyleColorSymbol::default(),
            ke_key_config: Keys::default(),
            #[cfg(feature = "mpris")]
            mpris: mpris::Mpris::default(),
            #[cfg(feature = "discord")]
            discord,
            db,
            layout: TermusicLayout::TreeView,
            config_layout: ConfigEditorLayout::General,
            db_criteria,
            db_search_results: Vec::new(),
            db_search_tracks: Vec::new(),
            config_changed: false,
            podcasts,
            podcasts_index: 0,
            db_podcast,
            threadpool,
            tx_to_main,
            rx_to_main,
            download_tracker: DownloadTracker::default(),
            podcast_search_vec: None,
        }
    }

    pub fn get_full_path_from_config(config: &Settings) -> PathBuf {
        let mut full_path = String::new();
        if let Some(dir) = config.music_dir.get(0) {
            full_path = shellexpand::tilde(dir).to_string();
        }

        if let Some(music_dir) = &config.music_dir_from_cli {
            full_path = shellexpand::tilde(music_dir).to_string();
        };
        PathBuf::from(full_path)
    }

    pub fn init_config(&mut self) {
        if let Err(e) = Self::theme_select_save() {
            self.mount_error_popup(format!("theme save error: {e}"));
        }
        self.mount_label_help();
        self.db.sync_database(&self.path);
        self.playlist_sync();
    }

    /// Initialize terminal
    pub fn init_terminal(&mut self) {
        let _drop = self.terminal.enable_raw_mode();
        let _drop = self.terminal.enter_alternate_screen();
        let _drop = self.terminal.clear_screen();
    }

    /// Finalize terminal
    pub fn finalize_terminal(&mut self) {
        let _drop = self.terminal.disable_raw_mode();
        let _drop = self.terminal.leave_alternate_screen();
        let _drop = self.terminal.clear_screen();
    }
    /// Returns elapsed time since last redraw
    pub fn since_last_redraw(&self) -> Duration {
        self.last_redraw.elapsed()
    }
    pub fn force_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn run(&mut self) {
        if self.player.playlist.is_stopped() {
            self.player.start_play();
            self.player_restore_last_position();
        }
    }

    pub fn player_stop(&mut self) {
        self.time_pos = 0;
        self.player.stop();
        self.player
            .message_tx
            .send(crate::player::PlayerMsg::Progress(0, 60))
            .ok();
        if let Err(e) = self.update_photo() {
            self.mount_error_popup(format!("update photo error: {e}"));
        };
        self.progress_update_title();
        self.lyric_update_title();
        self.lyric_update();
        self.force_redraw();
    }

    pub fn player_update_current_track_after(&mut self) {
        #[cfg(any(feature = "mpris", feature = "discord"))]
        if let Some(song) = self.player.playlist.current_track() {
            #[cfg(feature = "mpris")]
            self.mpris.add_and_play(song);
            #[cfg(feature = "discord")]
            if !self.config.disable_discord_rpc_from_cli {
                self.discord.update(song);
            }
        }
        self.time_pos = 0;
        self.playlist_sync();
        if let Err(e) = self.update_photo() {
            self.mount_error_popup(format!("update photo error: {e}"));
        };
        self.progress_update_title();
        self.lyric_update_title();
        self.update_playing_song();
    }

    pub fn player_previous(&mut self) {
        if let Loop::Single | Loop::Queue = self.config.loop_mode {
            return;
        }

        if self.player.playlist.is_empty() {
            self.player_stop();
            return;
        }

        self.player.playlist.handle_previous();
        self.player.skip();
    }

    pub fn player_toggle_pause(&mut self) {
        if self.player.playlist.is_empty() && self.player.playlist.current_track().is_none() {
            return;
        }
        if self.player.is_paused() {
            self.player.resume();
            #[cfg(feature = "mpris")]
            self.mpris.resume();
            #[cfg(feature = "discord")]
            self.discord.resume(self.time_pos);
        } else {
            self.player.pause();
            #[cfg(feature = "mpris")]
            self.mpris.pause();
            #[cfg(feature = "discord")]
            self.discord.pause();
        }
        self.progress_update_title();
    }

    pub fn player_seek(&mut self, offset: i64) {
        // FIXME: dirty fix for seeking when paused with symphonia,basically set it to play
        // in rusty sink code, and seek, and then set it back to pause.
        #[cfg(not(any(feature = "mpv", feature = "gst")))]
        let paused = self.player.is_paused();
        #[cfg(not(any(feature = "mpv", feature = "gst")))]
        if paused {
            self.player.set_volume(0);
        }

        self.player.seek(offset).ok();

        #[cfg(not(any(feature = "mpv", feature = "gst")))]
        if paused {
            self.force_redraw();
            std::thread::sleep(std::time::Duration::from_millis(50));
            self.player.pause();
            self.player.set_volume(self.config.volume);
        }
    }

    #[allow(clippy::cast_sign_loss)]
    pub fn player_save_last_position(&mut self) {
        match self.config.remember_last_played_position {
            crate::config::LastPosition::Yes => {
                if let Some(track) = self.player.playlist.current_track() {
                    match track.media_type {
                        Some(MediaType::Music) => self
                            .db
                            .set_last_position(track, Duration::from_secs(self.time_pos as u64)),
                        Some(MediaType::Podcast) => self
                            .db_podcast
                            .set_last_position(track, Duration::from_secs(self.time_pos as u64)),
                        None => {}
                    }
                }
            }
            crate::config::LastPosition::No => {}
            crate::config::LastPosition::Auto => {
                if let Some(track) = self.player.playlist.current_track() {
                    // 10 minutes
                    if track.duration().as_secs() >= 600 {
                        match track.media_type {
                            Some(MediaType::Music) => self.db.set_last_position(
                                track,
                                Duration::from_secs(self.time_pos as u64),
                            ),
                            Some(MediaType::Podcast) => self.db_podcast.set_last_position(
                                track,
                                Duration::from_secs(self.time_pos as u64),
                            ),
                            None => {}
                        }
                    }
                }
            }
        }
    }

    // #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub fn player_restore_last_position(&mut self) {
        let mut restored = false;
        match self.config.remember_last_played_position {
            crate::config::LastPosition::Yes => {
                if let Some(track) = self.player.playlist.current_track() {
                    match track.media_type {
                        Some(MediaType::Music) => {
                            if let Ok(last_pos) = self.db.get_last_position(track) {
                                self.player.seek_to(last_pos);
                                restored = true;
                            }
                        }

                        Some(MediaType::Podcast) => {
                            if let Ok(last_pos) = self.db_podcast.get_last_position(track) {
                                self.player.seek_to(last_pos);
                                restored = true;
                            }
                        }
                        None => {}
                    }
                }
            }
            crate::config::LastPosition::No => {}
            crate::config::LastPosition::Auto => {
                if let Some(track) = self.player.playlist.current_track() {
                    // 10 minutes
                    if track.duration().as_secs() >= 600 {
                        match track.media_type {
                            Some(MediaType::Music) => {
                                if let Ok(last_pos) = self.db.get_last_position(track) {
                                    self.player.seek_to(last_pos);
                                    restored = true;
                                }
                            }

                            Some(MediaType::Podcast) => {
                                if let Ok(last_pos) = self.db_podcast.get_last_position(track) {
                                    self.player.seek_to(last_pos);
                                    restored = true;
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
        }

        if restored {
            if let Some(track) = self.player.playlist.current_track() {
                self.db.set_last_position(track, Duration::from_secs(0));
            }
        }
    }
}
