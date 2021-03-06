use crate::{support, InstallerConfig};
use glium::glutin::{Event, WindowEvent};
use imgui::*;

use crate::archive_extractor::{ArchiveExtractor, ExtractionStatus};
use crate::process_runner::ProcessRunner;
use crate::support::ApplicationGUI;
use crate::windows_utilities;

const MOUSE_ACTIVITY_TIMEOUT_SECS: u64 = 1;

pub struct TimeoutTimer {
	last_refresh: std::time::Instant,
	timeout: std::time::Duration,
}

impl TimeoutTimer {
	pub fn new(timeout: std::time::Duration) -> TimeoutTimer {
		TimeoutTimer {
			last_refresh: std::time::Instant::now(),
			timeout,
		}
	}

	pub fn refresh(&mut self) {
		self.last_refresh = std::time::Instant::now();
	}

	pub fn expired(&self) -> bool {
		return std::time::Instant::now().saturating_duration_since(self.last_refresh)
			> self.timeout;
	}
}

// Extension methods for imgui-rs
pub trait SimpleUI {
	fn simple_button(&self, label: &ImStr) -> bool;
	fn show_developer_tools(&self);
	fn text_red<T: AsRef<str>>(&self, text: T);
	fn text_yellow<T: AsRef<str>>(&self, text: T);
}

impl<'ui> SimpleUI for Ui<'ui> {
	fn simple_button(&self, label: &ImStr) -> bool {
		self.button(label, [0.0f32, 0.0f32])
	}

	fn show_developer_tools(&self) {
		let mut show_demo_window = true;
		let mut show_metrics_window = true;
		self.show_demo_window(&mut show_demo_window);
		self.show_metrics_window(&mut show_metrics_window);
		self.show_default_style_editor();
	}

	fn text_red<T: AsRef<str>>(&self, text: T) {
		self.text_colored([1.0, 0.0, 0.0, 1.0], text);
	}

	fn text_yellow<T: AsRef<str>>(&self, text: T) {
		self.text_colored([1.0, 1.0, 0.0, 1.0], text);
	}
}

pub struct InstallStartedState {
	pub python_monitor: ProcessRunner,
	pub is_graphical: bool,
}

impl InstallStartedState {
	pub fn new(python_monitor: ProcessRunner, is_graphical: bool) -> InstallStartedState {
		InstallStartedState {
			python_monitor,
			is_graphical,
		}
	}
}

pub struct InstallFailedState {
	pub failure_reason: String,
	pub console_window_displayed: bool,
}

impl InstallFailedState {
	pub fn new(failure_reason: String) -> InstallFailedState {
		InstallFailedState {
			failure_reason,
			console_window_displayed: false,
		}
	}
}

pub struct ExtractingPythonState {
	pub extractor: ArchiveExtractor,
	pub progress_percentage: usize,
}

impl ExtractingPythonState {
	pub fn new(force_extraction: bool) -> ExtractingPythonState {
		ExtractingPythonState {
			extractor: ArchiveExtractor::new(force_extraction),
			progress_percentage: 0,
		}
	}
}

pub enum InstallerProgression {
	ExtractingPython(ExtractingPythonState),
	WaitingUserPickInstallType,
	InstallStarted(InstallStartedState),
	InstallFinished,
	InstallFailed(InstallFailedState),
}

pub struct InstallerState {
	// Installer state which depends on your progression through the installer
	pub progression: InstallerProgression,
	// If there is any installer state which doesn't depend on your current progression through the
	// installer, it should be put here.
}

impl InstallerState {
	pub fn new() -> InstallerState {
		InstallerState {
			progression: InstallerProgression::ExtractingPython(ExtractingPythonState::new(false)),
		}
	}
}

struct UIState {
	window_size: [f32; 2],
	show_developer_tools: bool,
	show_console: bool,
	close_requested: bool,
	// If 'run' is false at the end of a frame, the program will exit
	run: bool,
	// A timer which times out if user hasn't moved their mouse for a while over this program
	mouse_activity_timer: TimeoutTimer,
	// True when this program has focus, false otherwise (not individual ImGUI windows)
	program_is_focused: bool,
	// True if the user ticked the Safe-Mode checkbox
	safe_mode_enabled: bool,
}

impl UIState {
	pub fn new(window_size: [f32; 2]) -> UIState {
		UIState {
			window_size,
			show_developer_tools: false,
			show_console: false,
			close_requested: false,
			run: true,
			mouse_activity_timer: TimeoutTimer::new(std::time::Duration::from_secs(
				MOUSE_ACTIVITY_TIMEOUT_SECS,
			)),
			program_is_focused: true,
			safe_mode_enabled: false,
		}
	}
}

struct InstallerGUI {
	// State which mainly relates to the presentation of the UI (ViewModel?)
	ui_state: UIState,
	// State which mainly relates to the execution of the installer
	state: InstallerState,
	// Configuration information which doesn't change during the course of the install is put here
	config: InstallerConfig,
}

impl InstallerGUI {
	pub fn new(window_size: [f32; 2], constants: InstallerConfig) -> InstallerGUI {
		InstallerGUI {
			ui_state: UIState::new(window_size),
			state: InstallerState::new(),
			config: constants,
		}
	}

	pub fn display_ui(&mut self, ui: &Ui) {
		ui.new_line();

		// Update the installer based on the extraction status
		self.extraction_update();

		// Handle when user attempts to close the program
		self.exit_handler(ui);

		// Main installer flow allowing user to progress through the installer
		self.display_main_installer_flow(ui);

		ui.new_line();

		// Show the advanced tools section
		self.display_advanced_tools(ui);
	}

	// Monitor extraction and advance the installer state once extraction finished
	fn extraction_update(&mut self) {
		if let InstallerProgression::ExtractingPython(extraction_state) =
			&mut self.state.progression
		{
			match extraction_state.extractor.poll_status() {
				ExtractionStatus::NotStarted => extraction_state
					.extractor
					.start_extraction(self.config.sub_folder),
				ExtractionStatus::Started(Some(progress)) => {
					extraction_state.progress_percentage = progress;
				}
				ExtractionStatus::Started(None) => {}
				ExtractionStatus::Finished => {
					self.state.progression = InstallerProgression::WaitingUserPickInstallType;
				}
				ExtractionStatus::Error(error_str) => {
					self.state.progression =
						InstallerProgression::InstallFailed(InstallFailedState::new(String::from(error_str)))
				}
			}
		}
	}

	// This modal prevents users accidentally terminating the install before it has finished
	// If python is extracting or installation has started, show a popup for user to confirm exit
	// In any other case, just let the user exit immediately
	fn exit_handler(&mut self, ui: &Ui) {
		let confirm_exit_modal_name = im_str!("Confirm Exit");
		if self.ui_state.close_requested {
			self.ui_state.close_requested = false;
			match self.state.progression {
				InstallerProgression::ExtractingPython(_)
				| InstallerProgression::WaitingUserPickInstallType => self.quit(),
				_ => ui.open_popup(confirm_exit_modal_name),
			}
		}

		// Exit confirmation modal triggered by the above
		ui.popup_modal(confirm_exit_modal_name).build(|| {
			ui.text("Closing this window will terminate the installer!");
			if ui.button(im_str!("OK - Quit Installer"), [0.0, 0.0]) {
				ui.close_current_popup();
				self.quit();
			}
			if ui.button(im_str!("Cancel"), [0.0, 0.0]) {
				ui.close_current_popup();
			}
		});
	}

	fn display_main_installer_flow(&mut self, ui: &Ui) {
		// Display parts of the UI which depend on the installer progression
		match &mut self.state.progression {
			InstallerProgression::ExtractingPython(extraction_state) => {
				ui.text_yellow(im_str!("Please wait for extraction to finish..."));
				ProgressBar::new((extraction_state.progress_percentage as f32) / 100.0f32)
					.size([500.0, 24.0])
					.overlay_text(&im_str!(
						"{}% extracted",
						extraction_state.progress_percentage
					))
					.build(&ui);
			}
			InstallerProgression::WaitingUserPickInstallType => {
				ui.text_red(im_str!("Please click 'Run Installer'"));
				ui.text_red(im_str!(
					"If you have problems, enable 'Run in Safe-Mode' for the text-based installer"
				));

				let install_button_clicked = ui.simple_button(im_str!("Run Installer"));
				ui.same_line_with_spacing(0., 20.);
				ui.checkbox(im_str!("Run in Safe-Mode"), &mut self.ui_state.safe_mode_enabled);
				if install_button_clicked {
					if let Err(e) = self.start_install(!self.ui_state.safe_mode_enabled) {
						println!("Failed to start install! {:?}", e)
					}
				}
			}
			InstallerProgression::InstallStarted(graphical_install) => {
				if graphical_install.is_graphical {
					ui.text_yellow(im_str!(
						"Please wait - Installer will launch in your web browser"
					));
					ui.text_red(im_str!(
						"If the web browser fails to load, restart this launcher, then enable the 'Run in Safe-Mode' option"
					));
				} else {
					ui.text_yellow(im_str!(
						"Console Installer Started - Please use the console window that just opened."
					));
				}

				if graphical_install.python_monitor.task_has_failed_nonblocking() {
					self.state.progression = InstallerProgression::InstallFailed(
						InstallFailedState::new(String::from("Python Installer Failed - See Console Window"))
					)
				}
			}
			InstallerProgression::InstallFinished => {
				ui.text_yellow(im_str!("Please wait - Installer is closing"));
			}
			InstallerProgression::InstallFailed(install_failed_state) => {
				ui.text_red(im_str!("The installation failed!"));
				ui.text_red(im_str!("[{}]", install_failed_state.failure_reason));

				if !install_failed_state.console_window_displayed {
					windows_utilities::show_console_window();
					install_failed_state.console_window_displayed = true;
				}
			}
		};
	}

	// Advanced tools used if something went wrong. Hidden by default untlick you expend the header
	fn display_advanced_tools(&mut self, ui: &Ui) {
		// Advanced Tools Section
		if ui.collapsing_header(im_str!("Advanced Tools")).build() {
			// Button which shows the python installer logs folder.
			// NOTE: the output of this launcher is currently not logged.
			if ui.button(im_str!("Show Installer Logs"), [0., 0.]) {
				let _ = windows_utilities::system_open(&self.config.logs_folder);
			}

			// Button which forces re-extraction
			match self.state.progression {
				InstallerProgression::WaitingUserPickInstallType
				| InstallerProgression::InstallFinished
				| InstallerProgression::InstallFailed(_) => {
					ui.same_line(0.);
					if ui.simple_button(im_str!("Force Re-Extraction")) {
						self.state.progression = InstallerProgression::ExtractingPython(
							ExtractingPythonState::new(true),
						);
					}
				}
				_ => {}
			}

			// Show windows' 'cmd' console
			if ui.checkbox(
				im_str!("Show Debug Console"),
				&mut self.ui_state.show_console,
			) {
				if self.ui_state.show_console {
					windows_utilities::show_console_window();
				} else {
					windows_utilities::hide_console_window();
				}
			}
			ui.same_line(0.);

			// Show ImGUI Developer tools (and any other tools)
			ui.checkbox(
				im_str!("Show Developer Tools"),
				&mut self.ui_state.show_developer_tools,
			);
			ui.same_line(0.);
		}
	}

	// Start either the graphical or console install. Advances the installer progression to "InstallStarted"
	fn start_install(&mut self, is_graphical: bool) -> Result<(), Box<dyn std::error::Error>> {
		let script_name = if is_graphical {
			"main.py"
		} else {
			// Interactive CLI installer needs console visible so user can see and type into it.
			windows_utilities::show_console_window();
			"cli_interactive.py"
		};

		let python_monitor = ProcessRunner::new(
			&self.config.python_path,
			self.config.sub_folder,
			&["-u", "-E", script_name],
		)?;

		self.state.progression = InstallerProgression::InstallStarted(InstallStartedState::new(
			python_monitor,
			is_graphical,
		));

		Ok(())
	}

	// Close the UI and the installer thread
	fn quit(&mut self) {
		self.ui_state.run = false;

		// Attempt to kill the python process, if the installer has already been started.
		// Even if killing fails, attempt to wait on the process.
		// This will make it obvious if something went wrong as the UI will (probably) hang,
		// so the user can close the program using task manager.
		if let InstallerProgression::InstallStarted(settings) = &mut self.state.progression {
			let _ = settings.python_monitor.kill_wait();
		}

		self.state.progression = InstallerProgression::InstallFinished;
	}

	// Power saving mode is determined by the following
	// - If the program has focus, power saving mode is disabled
	// - If no mouse activity is detected for MOUSE_ACTIVITY_TIMEOUT_SECS seconds,
	//   power saving mode is disabled
	// - Otherwise, power saving mode is enabled.
	fn should_save_power(&self) -> bool {
		!self.ui_state.program_is_focused && self.ui_state.mouse_activity_timer.expired()
	}
}

impl ApplicationGUI for InstallerGUI {
	fn ui_loop(&mut self, ui: &mut Ui) -> bool {
		// Prevent high cpu/gpu usage due to unlimited framerate when window minimized on Windows
		// as well as generally reducing usage if the user isn't using the program
		if self.should_save_power() {
			std::thread::sleep(std::time::Duration::from_millis(100));
		}

		let unround_style = ui.push_style_var(StyleVar::WindowRounding(0.0));

		// Hide developer tools by default
		if self.ui_state.show_developer_tools {
			ui.show_developer_tools();
		}

		// Main window containing the installer
		Window::new(im_str!("07th-Mod Installer Launcher"))
			.position([0.0, 0.0], Condition::Always)
			.size(self.ui_state.window_size, Condition::Always)
			.no_decoration() //remove title bar etc. so it acts like the "Main" window of the program
			.build(ui, || {
				self.display_ui(ui);
			});

		unround_style.pop(&ui);

		return self.ui_state.run;
	}

	fn handle_event(&mut self, event: Event) {
		match &event {
			Event::WindowEvent {
				window_id: _,
				event,
			} => match event {
				WindowEvent::Focused(focused) => self.ui_state.program_is_focused = *focused,
				WindowEvent::CursorMoved { .. } => self.ui_state.mouse_activity_timer.refresh(),
				WindowEvent::CloseRequested => self.ui_state.close_requested = true,
				_ => {}
			},
			_ => {}
		}
	}
}

pub fn ui_loop() {
	let window_size = [1000., 500.];
	let system = support::init(
		InstallerGUI::new(
			[window_size[0] as f32, window_size[1] as f32],
			InstallerConfig::new(),
		),
		concat!("07th-Mod Installer Launcher [", env!("TRAVIS_TAG"), "]"),
		window_size,
	);

	system.main_loop();
}
