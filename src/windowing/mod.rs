// #[cfg(feature = "ico")]
// use ico::{IconDir, IconDirEntry, IconImage, ResourceType};

use ::minifb;

#[cfg(target_os = "windows")]
use mirl_buffer::Buffer;

use mirl_core::platform::{KeyCode, KeyboardState};
// #[cfg(feature = "ico")]
// use crate::graphics::u32_to_rgba_u8;
use mirl_extensions::*;
use mirl_input::mouse::{
    DefaultCursorColorInfo, LoadCursorError, LoadDefaultCustomCursor, MouseButton,
    cursors::{CursorResolution, DefaultCursors, RawCursor},
};
use mirl_system::{
    Os,
    cursors::{CursorType, UseCursorError},
    traits::*,
};

use mirl_windowing_core::windowing::{WindowSettings, errors::*, traits::*};

/// Backend implementation using `MiniFB`
#[derive(Debug)]
#[cfg_attr(feature = "c_compatible", repr(C))]
pub struct Framework {
    window: ::minifb::Window,
    cursor_subclassed: bool,
    keyboard_state: KeyboardState,
}
#[must_use]
/// Convert [`WindowSettings`] to [`WindowOptions`](minifb::WindowOptions)
pub fn minifb_window_options_from_options(
    window_options: &WindowSettings,
) -> ::minifb::WindowOptions {
    ::minifb::WindowOptions {
        borderless: window_options.borderless,
        title: window_options.title_visible,
        resize: window_options.resizable,
        scale: ::minifb::Scale::X1,
        scale_mode: ::minifb::ScaleMode::Stretch,
        topmost: window_options.window_level == WindowRenderLayer::AlwaysOnTop,
        transparency: false,
        none: false,
    }
}
#[must_use]
/// Translate the minifb string error into an enum variant is possible
pub fn minifb_window_creation_error_to_error(error: &str) -> WindowCreationError {
    match error {
        "Window transparency requires the borderless property" => {
            WindowCreationError::TransparencyRequiresBorderlessProperty
        }
        "Unable to create Window" => WindowCreationError::OsFailed,
        _ => WindowCreationError::Misc(error.to_string()),
    }
}
#[must_use]
/// The update error "Buffer too small" string is a little more complicated because it's a formatted string
pub fn parse_update_buffer_size_mismatch_error(
    input: &str,
) -> Option<(usize, usize, usize, usize, usize)> {
    let prefix = "Update failed because input buffer is too small. Required size for ";
    let rest = input.strip_prefix(prefix)?;

    let mut parts = rest.split(' ');

    let buf_width = parts.next()?.parse().ok()?;
    let buf_stride = parts
        .next()?
        .trim_start_matches('(')
        .trim_end_matches(" stride)")
        .parse()
        .ok()?;

    let after_stride = parts.collect::<Vec<_>>().join(" ");
    let after_stride = after_stride.strip_prefix("x ")?;

    let mut p = after_stride.split(" buffer is ");
    let buf_height = p.next()?.parse().ok()?;
    let rest2 = p.next()?;

    let mut p2 = rest2.split(" bytes but the size of the input buffer has the size ");
    let required = p2.next()?.parse().ok()?;
    let actual = p2.next()?.trim_end_matches(" bytes").parse().ok()?;

    Some((buf_width, buf_stride, buf_height, required, actual))
}
#[must_use]
/// Convert the given update error string into an [enum variant](WindowUpdateError)
pub fn minifb_window_update_error_to_error(error: &str) -> WindowUpdateError {
    parse_update_buffer_size_mismatch_error(error).map_or_else(
        || {
            #[allow(clippy::match_single_binding)]
            match error {
                _ => WindowUpdateError::Misc(error.to_string()),
            }
        },
        |val| WindowUpdateError::BufferInvalidSize {
            width: val.0,
            stride: val.1,
            height: val.2,
            expected: val.3,
            gotten: val.4,
        },
    )
}

#[must_use]
/// Convert the given given minifb error into the mirl equitant
pub fn minifb_error_to_error(error: &minifb::Error) -> WindowError {
    match error {
        minifb::Error::MenuExists(_) => WindowError::DuplicateWindow,
        minifb::Error::MenusNotSupported => WindowError::OsNotSupported,
        minifb::Error::WindowCreate(reason) => {
            WindowError::FailedToOpenWindow(minifb_window_creation_error_to_error(reason))
        }
        minifb::Error::UpdateFailed(reason) => {
            WindowError::FailedToUpdateWindow(minifb_window_update_error_to_error(reason))
        }
    }
}

impl NewWindow for Framework {
    /// ### Create a new window with the desired settings
    ///
    /// ### Inputs:
    /// `title`: How the window should be named regardless of if it's shown
    ///
    /// `settings`: See [`WindowSettings`](crate::prelude::WindowSettings) for more info
    // /// `cursor`: If you wish to use cursors other than the default one, provide the cursor you want the window to show by default. If this is set to None, [`set_cursor_style()`](ExtendedWindow::set_cursor_style) may not work as intended
    /// # Errors
    /// See [`WindowError`] for the error messages
    ///
    /// # Framework Specific:
    /// Settings not accounted for:
    ///
    /// visible
    fn new(title: &str, settings: WindowSettings) -> Result<Self, WindowError> {
        let width = settings.size.0;
        let height = settings.size.1;
        let mut window = match minifb::Window::new(
            title,
            width as usize,
            height as usize,
            minifb_window_options_from_options(&settings),
        ) {
            Ok(w) => w,
            Err(er) => {
                return Err(minifb_error_to_error(&er));
            }
        };

        window.set_position(settings.position.0 as isize, settings.position.1 as isize);
        Os::set_window_borderless(
            &get_native_window_handle_from_minifb(&window),
            settings.borderless,
        );
        Os::set_window_level(
            &get_native_window_handle_from_minifb(&window),
            settings.window_level,
        );
        Ok(Self {
            window,
            cursor_subclassed: false,
            keyboard_state: KeyboardState::new(),
        })
    }
}
impl Window for Framework {
    #[inline]
    fn update_raw(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
    ) -> Result<(), WindowError> {
        //let s = self.window.get_size();
        match self.window.update_with_buffer(buffer, width, height) {
            Ok(()) => Ok(()),
            Err(e) => Err(minifb_error_to_error(&e)),
        }
    }

    #[inline]
    fn is_open(&self) -> bool {
        self.window.is_open()
    }

    fn close_and_clean_up(&mut self) {
        //TODO: ADD DESTROYER
    }
}

impl MouseInput for Framework {
    #[inline]
    fn get_mouse_position(&self) -> Option<(f32, f32)> {
        let value = self
            .window
            .get_unscaled_mouse_pos(minifb::MouseMode::Pass)?;

        value.try_tuple_into()
    }
    #[inline]
    fn is_mouse_down(&self, button: MouseButton) -> bool {
        if let Some(key) = map_mouse_button_to_minifb(button) {
            return self.window.get_mouse_down(key);
        }
        false
    }
}
impl KeyboardInput for Framework {
    #[inline]
    fn is_key_down(&self, key: KeyCode) -> bool {
        self.window.is_key_down(map_keycode_to_minifb(key))
    }
}

impl Output for Framework {
    #[inline]
    fn log(&self, t: &str) {
        t.println_self();
    }
}

// impl Timing for Framework {
//     #[inline]
//     fn get_time(&self) -> std::time::Instant {
//         std::time::Instant::now()
//     }
//     // #[inline]
//     // fn get_delta_time(&mut self) -> f64 {
//     //     let (time, r) = shared::sample_fps(&self.time);
//     //     self.time = time;
//     //     r
//     // }
//     #[inline]
//     fn sleep(&self, time: std::time::Duration) {
//         shared::sleep(time);
//     }
// }
impl RenderLayer for Framework {
    #[inline]
    fn set_render_layer(&mut self, level: WindowRenderLayer) {
        Os::set_window_level(&self.get_window_handle(), level);
    }
}
impl Visibility for Framework {
    #[inline]
    fn maximize(&mut self) {
        Os::maximize(&self.get_window_handle());
    }
    #[inline]
    fn minimize(&mut self) {
        Os::minimize(&self.get_window_handle());
    }
    #[inline]
    fn restore(&mut self) {
        Os::restore(&self.get_window_handle());
    }
    fn is_maximized(&self) -> bool {
        Os::is_maximized(&self.get_window_handle())
    }
    fn is_minimized(&self) -> bool {
        Os::is_minimized(&self.get_window_handle())
    }
}

impl ExtendedMouseInput for Framework {
    #[inline]
    fn get_mouse_scroll(&self) -> (f32, f32) {
        self.window.get_scroll_wheel().unwrap_or_default()
    }
}

impl ExtendedKeyboardInput for Framework {
    fn get_all_keys_down(&self) -> Vec<KeyCode> {
        self.keyboard_state.get_all_pressed_keys()
    }
}

impl ManageCursorStyle<mirl_system::cursors::NativeCursor> for Framework {
    fn load_custom_default_cursors(
        &mut self,
        size: CursorResolution,
        color_info: DefaultCursorColorInfo,
    ) -> Result<DefaultCursors<mirl_system::cursors::NativeCursor>, LoadCursorError> {
        mirl_system::cursors::NativeCursor::get_default_custom_cursors(size, color_info)
    }
    fn load_custom_cursor(
        &mut self,
        cursor: RawCursor,
    ) -> Result<mirl_system::cursors::NativeCursor, LoadCursorError> {
        mirl_system::cursors::NativeCursor::new(cursor)
    }
    #[inline]
    fn set_cursor_style(
        &mut self,
        style: &mirl_system::cursors::NativeCursor,
    ) -> Result<(), UseCursorError> {
        #[cfg(target_os = "windows")]
        {
            if !self.cursor_subclassed {
                unsafe {
                    use mirl_system::cursors::windows::subclass_window;

                    subclass_window(self.get_window_handle(), &style.cursor);
                }
                self.cursor_subclassed = true;
            }
        }
        style.use_cursor(())
    }
}

// impl BufferScaling for Framework {
//     fn get_scale_level(&self) -> super::WindowScale {}
//     fn set_scale_level(&mut self, scale: super::WindowScale) {
//         self.window.sca
//     }
// }

impl ExtendedTiming for Framework {
    #[inline]
    fn set_target_fps(&mut self, fps: usize) {
        self.window.set_target_fps(fps);
    }
}

impl IconControl for Framework {
    #[inline]
    fn set_icon(&mut self, buffer: &mirl_buffer::Buffer) -> Result<(), WindowError> {
        // assert_eq!(
        //     buffer.len(),
        //     (width * height) as usize,
        //     "Buffer size doesn't match dimensions"
        // );
        // let buffer64: Vec<u64> = buffer.iter().map(|&x| x as u64).collect();

        // let boxed_buffer = buffer64.into_boxed_slice();

        // // Leak the memory (intentionally) so it persists for the lifetime of the application or whatever the hell that means
        // let leaked_buffer = Box::leak(boxed_buffer);

        // let icon = minifb::Icon::Buffer(
        //     leaked_buffer.as_ptr(),
        //     leaked_buffer.len() as u32,
        // );
        // self.window.set_icon(icon);
        #[cfg(target_os = "windows")]
        {
            // TODO: Use tempfile crate instead of std::env::temp_dir

            use std::str::FromStr;

            use mirl_input::formats::BufferToIco;

            let Some(ico_data) = buffer.create_ico() else {
                return Err(WindowError::Misc(
                    "Unable to convert image into .ico, maybe check it's size".to_string(),
                ));
            };

            let temp_dir = std::env::temp_dir();
            let ico_path = temp_dir.join("temp_icon.ico");
            let string_path = match ico_path.to_str() {
                Some(v) => v.to_string(),
                None => {
                    return Err(WindowError::Misc(format!(
                        "Unable to convert '{}' to string",
                        ico_path.display()
                    )));
                }
            };

            match std::fs::write(&ico_path, &ico_data) {
                Ok(()) => {}
                Err(_) => {
                    return Err(WindowError::FileAccessNotPossible { path: string_path });
                }
            }
            if let Ok(p) = minifb::Icon::from_str(&string_path) {
                self.window.set_icon(p);
            } else {
                return Err(WindowError::UnableToLoadIcon);
            }

            return Ok(());
        }

        // For non-Windows platforms, try the buffer approach
        #[cfg(not(target_os = "windows"))]
        {
            let buffer64: Vec<u64> = buffer.iter().map(|&x| x as u64).collect();
            let boxed_buffer = buffer64.into_boxed_slice();
            let leaked_buffer = Box::leak(boxed_buffer);

            // Different platforms might expect different Icon constructor signatures
            // You might need to check minifb source to see exact parameters needed
            let icon = minifb::Icon::Buffer(
                leaked_buffer.as_ptr(),
                leaked_buffer.len() as u32,
                width,
                height,
            );

            self.window.set_icon(icon);
            return WindowError::AllGood;
        }
        #[allow(unused)]
        return Err(WindowError::NotImplemented);
    }
}

impl GetWindowHandle for Framework {
    fn get_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        get_native_window_handle_from_minifb(&self.window)
    }
}
impl ExtendedWindow for Framework {
    #[inline]
    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }
    // #[inline]
    // fn wait(&self, time: u64) {
    //     std::thread::sleep(Duration::from_millis(time));
    // }
}

fn get_native_window_handle_from_minifb(
    window: &minifb::Window,
) -> raw_window_handle::RawWindowHandle {
    let window_handle = window.get_window_handle();

    #[cfg(target_os = "windows")]
    {
        let handle = raw_window_handle::Win32WindowHandle::new(
            // The window handle cannot be 0
            unsafe { core::num::NonZero::new(window_handle as isize).unwrap_unchecked() },
        );
        raw_window_handle::RawWindowHandle::Win32(handle)
    }
    #[cfg(target_os = "macos")]
    {
        let mut handle = raw_window_handle::AppKitWindowHandle::empty();
        handle.ns_view = window_handle;
        raw_window_handle::RawWindowHandle::AppKit(handle)
    }

    // #[cfg(all(target_os = "linux", not(feature = "wayland")))]
    // {
    //     let mut handle = raw_window_handle::XlibWindowHandle::empty();
    //     handle.window = window_handle;
    //     //handle.display = window.get_x11_display().cast();
    //     raw_window_handle::RawWindowHandle::Xlib(handle)
    // }

    // #[cfg(all(target_os = "linux", feature = "wayland"))]
    // {
    //     let mut handle = raw_window_handle::WaylandWindowHandle::empty();
    //     handle.surface = window_handle;
    //     //handle.display = window.get_wayland_display();
    //     raw_window_handle::RawWindowHandle::Wayland(handle)
    // }
}

#[cfg(target_os = "windows")]
impl Control for Framework {
    #[inline]
    fn set_size(&mut self, buffer: &Buffer) {
        Os::set_window_size(
            &self.get_window_handle(),
            (buffer.width as i32, buffer.height as i32),
        );
    }
    #[inline]
    fn get_size(&self) -> (i32, i32) {
        Os::get_window_size(&get_native_window_handle_from_minifb(&self.window))
    }
    #[inline]
    fn set_position(&mut self, xy: (i32, i32)) {
        self.window.set_position(xy.0 as isize, xy.1 as isize);
    }
    #[inline]
    fn get_position(&self) -> (i32, i32) {
        // MiniFB uses i32 before converting to isize meaning it is 100% safe to reconvert it to i32
        unsafe {
            self.window
                .get_position()
                .try_tuple_into()
                .unwrap_unchecked()
        }
    }
}

// #[cfg(feature = "ico")]
// fn encode_to_ico_format(
//     buffer: &[u32],
//     width: u32,
//     height: u32,
// ) -> Result<Vec<u8>, Box<dyn core::error::Error>> {
//     // Create a new icon directory
//     let mut icon_dir = IconDir::new(ResourceType::Icon);

//     // Convert the RGBA u32 buffer to a Vec<u8> in BGRA format
//     // Windows .ico format typically expects BGRA ordering
//     let mut image_data = Vec::with_capacity(buffer.len() * 4);

//     for &pixel in buffer {
//         // Extract RGBA components from u32
//         let (r, g, b, _a) = u32_to_rgba_u8(pixel);
//         // ALPHA IS NOT READ CORRECTLY -> IT'S ALWAYS 0
//         // println!("Fix alpha channel not being read correctly");

//         // Push as BGRA
//         image_data.push(r);
//         image_data.push(g);
//         image_data.push(b);
//         image_data.push(0);
//     }

//     // Create icon image with proper transparency
//     let icon_image = IconImage::from_rgba_data(width, height, image_data);

//     // Add the image to the icon directory
//     icon_dir.add_entry(IconDirEntry::encode(&icon_image)?);

//     // Encode the icon directory to a Vec<u8>
//     let mut ico_data = Vec::new();
//     icon_dir.write(&mut ico_data)?;

//     Ok(ico_data)
// }

// Compile-time key mapping function
// const fn map_cursor_style(style: CursorStyle) -> minifb::CursorStyle {
//     match style {
//         CursorStyle::Default => minifb::CursorStyle::Arrow,
//         CursorStyle::HandClosed => minifb::CursorStyle::HandClosed,
//         CursorStyle::HandOpen => minifb::CursorStyle::HandOpen,
//         CursorStyle::Insertion => minifb::CursorStyle::Ibeam,
//         CursorStyle::Crosshair => minifb::CursorStyle::Crosshair,
//         CursorStyle::ResizeHorizontal => minifb::CursorStyle::ResizeLeftRight,
//         CursorStyle::ResizeVertical => minifb::CursorStyle::ResizeUpDown,
//         CursorStyle::ResizeAll => minifb::CursorStyle::ResizeAll,
//     }
// }
/// Maps mirls `MouseButtons` to `MiniFBs` `MouseButtons`
const fn map_mouse_button_to_minifb(button: MouseButton) -> Option<minifb::MouseButton> {
    match button {
        MouseButton::Left => Some(minifb::MouseButton::Left),
        MouseButton::Right => Some(minifb::MouseButton::Right),
        MouseButton::Middle => Some(minifb::MouseButton::Middle),
        MouseButton::Extra1
        | MouseButton::Extra2
        | MouseButton::Extra3
        | MouseButton::Extra4
        | MouseButton::Unsupported => None,
    }
}
/// Maps mirls `KeyCodes` to `MiniFBs` Keycodes
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn map_keycode_to_minifb(key: KeyCode) -> minifb::Key {
    match key {
        // Letters
        KeyCode::KeyA => minifb::Key::A,
        KeyCode::KeyB => minifb::Key::B,
        KeyCode::KeyC => minifb::Key::C,
        KeyCode::KeyD => minifb::Key::D,
        KeyCode::KeyE => minifb::Key::E,
        KeyCode::KeyF => minifb::Key::F,
        KeyCode::KeyG => minifb::Key::G,
        KeyCode::KeyH => minifb::Key::H,
        KeyCode::KeyI => minifb::Key::I,
        KeyCode::KeyJ => minifb::Key::J,
        KeyCode::KeyK => minifb::Key::K,
        KeyCode::KeyL => minifb::Key::L,
        KeyCode::KeyM => minifb::Key::M,
        KeyCode::KeyN => minifb::Key::N,
        KeyCode::KeyO => minifb::Key::O,
        KeyCode::KeyP => minifb::Key::P,
        KeyCode::KeyQ => minifb::Key::Q,
        KeyCode::KeyR => minifb::Key::R,
        KeyCode::KeyS => minifb::Key::S,
        KeyCode::KeyT => minifb::Key::T,
        KeyCode::KeyU => minifb::Key::U,
        KeyCode::KeyV => minifb::Key::V,
        KeyCode::KeyW => minifb::Key::W,
        KeyCode::KeyX => minifb::Key::X,
        KeyCode::KeyY => minifb::Key::Y,
        KeyCode::KeyZ => minifb::Key::Z,

        // Numbers
        KeyCode::Num0 => minifb::Key::Key0,
        KeyCode::Num1 => minifb::Key::Key1,
        KeyCode::Num2 => minifb::Key::Key2,
        KeyCode::Num3 => minifb::Key::Key3,
        KeyCode::Num4 => minifb::Key::Key4,
        KeyCode::Num5 => minifb::Key::Key5,
        KeyCode::Num6 => minifb::Key::Key6,
        KeyCode::Num7 => minifb::Key::Key7,
        KeyCode::Num8 => minifb::Key::Key8,
        KeyCode::Num9 => minifb::Key::Key9,
        KeyCode::KeyPad0 => minifb::Key::NumPad0,
        KeyCode::KeyPad1 => minifb::Key::NumPad1,
        KeyCode::KeyPad2 => minifb::Key::NumPad2,
        KeyCode::KeyPad3 => minifb::Key::NumPad3,
        KeyCode::KeyPad4 => minifb::Key::NumPad4,
        KeyCode::KeyPad5 => minifb::Key::NumPad5,
        KeyCode::KeyPad6 => minifb::Key::NumPad6,
        KeyCode::KeyPad7 => minifb::Key::NumPad7,
        KeyCode::KeyPad8 => minifb::Key::NumPad8,
        KeyCode::KeyPad9 => minifb::Key::NumPad9,

        // Function Keys
        KeyCode::F1 => minifb::Key::F1,
        KeyCode::F2 => minifb::Key::F2,
        KeyCode::F3 => minifb::Key::F3,
        KeyCode::F4 => minifb::Key::F4,
        KeyCode::F5 => minifb::Key::F5,
        KeyCode::F6 => minifb::Key::F6,
        KeyCode::F7 => minifb::Key::F7,
        KeyCode::F8 => minifb::Key::F8,
        KeyCode::F9 => minifb::Key::F9,
        KeyCode::F10 => minifb::Key::F10,
        KeyCode::F11 => minifb::Key::F11,
        KeyCode::F12 => minifb::Key::F12,
        KeyCode::F13 => minifb::Key::F13,
        KeyCode::F14 => minifb::Key::F14,
        KeyCode::F15 => minifb::Key::F15,

        // Modifiers
        KeyCode::LeftShift => minifb::Key::LeftShift,
        KeyCode::RightShift => minifb::Key::RightShift,
        KeyCode::LeftControl => minifb::Key::LeftCtrl,
        KeyCode::RightControl => minifb::Key::RightCtrl,
        KeyCode::LeftAlt => minifb::Key::LeftAlt,
        KeyCode::RightAlt => minifb::Key::RightAlt,
        KeyCode::LeftSuper => minifb::Key::LeftSuper,
        KeyCode::RightSuper => minifb::Key::RightSuper,

        // Symbols
        KeyCode::Space => minifb::Key::Space,
        KeyCode::Enter | KeyCode::KeyPadEnter => minifb::Key::Enter,
        KeyCode::Escape => minifb::Key::Escape,
        KeyCode::Backspace => minifb::Key::Backspace,
        KeyCode::Tab => minifb::Key::Tab,

        // Arrows
        KeyCode::UpArrow => minifb::Key::Up,
        KeyCode::DownArrow => minifb::Key::Down,
        KeyCode::LeftArrow => minifb::Key::Left,
        KeyCode::RightArrow => minifb::Key::Right,

        // Extra
        KeyCode::Comma => minifb::Key::Comma,
        KeyCode::Period => minifb::Key::Period,
        KeyCode::Minus | KeyCode::KeyPadSubtract => minifb::Key::Minus,
        KeyCode::Equal => minifb::Key::Equal,
        KeyCode::LeftBracket => minifb::Key::LeftBracket,
        KeyCode::RightBracket => minifb::Key::RightBracket,
        KeyCode::Backslash => minifb::Key::Backslash,
        KeyCode::Semicolon => minifb::Key::Semicolon,
        KeyCode::Quote => minifb::Key::Apostrophe,
        // Other letters
        // Other
        KeyCode::ScrollLock => minifb::Key::ScrollLock,
        // Lock keys (new)
        KeyCode::CapsLock => minifb::Key::CapsLock,
        KeyCode::NumLock => minifb::Key::NumLock,

        // Editing keys
        KeyCode::Insert => minifb::Key::Insert,
        KeyCode::Delete => minifb::Key::Delete,
        KeyCode::Home => minifb::Key::Home,
        KeyCode::End => minifb::Key::End,
        KeyCode::PageUp => minifb::Key::PageUp,
        KeyCode::PageDown => minifb::Key::PageDown,

        // Keypad ops
        KeyCode::KeyPadDivide | KeyCode::Slash => minifb::Key::Slash,
        // Multimedia keys (unsupported in minifb)
        // Browser/OS keys
        // Platform-specific
        KeyCode::Menu => minifb::Key::Menu,
        KeyCode::Pause => minifb::Key::Pause,
        // Symbols not mapped before
        // Fallback
        _ => minifb::Key::Unknown,
    }
}

/// Maps `MiniFBs` `KeyCodes` to mirls Keycodes
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn map_minifb_to_keycode(key: minifb::Key) -> KeyCode {
    match key {
        // Letters
        minifb::Key::A => KeyCode::KeyA,
        minifb::Key::B => KeyCode::KeyB,
        minifb::Key::C => KeyCode::KeyC,
        minifb::Key::D => KeyCode::KeyD,
        minifb::Key::E => KeyCode::KeyE,
        minifb::Key::F => KeyCode::KeyF,
        minifb::Key::G => KeyCode::KeyG,
        minifb::Key::H => KeyCode::KeyH,
        minifb::Key::I => KeyCode::KeyI,
        minifb::Key::J => KeyCode::KeyJ,
        minifb::Key::K => KeyCode::KeyK,
        minifb::Key::L => KeyCode::KeyL,
        minifb::Key::M => KeyCode::KeyM,
        minifb::Key::N => KeyCode::KeyN,
        minifb::Key::O => KeyCode::KeyO,
        minifb::Key::P => KeyCode::KeyP,
        minifb::Key::Q => KeyCode::KeyQ,
        minifb::Key::R => KeyCode::KeyR,
        minifb::Key::S => KeyCode::KeyS,
        minifb::Key::T => KeyCode::KeyT,
        minifb::Key::U => KeyCode::KeyU,
        minifb::Key::V => KeyCode::KeyV,
        minifb::Key::W => KeyCode::KeyW,
        minifb::Key::X => KeyCode::KeyX,
        minifb::Key::Y => KeyCode::KeyY,
        minifb::Key::Z => KeyCode::KeyZ,

        // Numbers
        minifb::Key::Key0 => KeyCode::Num0,
        minifb::Key::Key1 => KeyCode::Num1,
        minifb::Key::Key2 => KeyCode::Num2,
        minifb::Key::Key3 => KeyCode::Num3,
        minifb::Key::Key4 => KeyCode::Num4,
        minifb::Key::Key5 => KeyCode::Num5,
        minifb::Key::Key6 => KeyCode::Num6,
        minifb::Key::Key7 => KeyCode::Num7,
        minifb::Key::Key8 => KeyCode::Num8,
        minifb::Key::Key9 => KeyCode::Num9,
        minifb::Key::NumPad0 => KeyCode::KeyPad0,
        minifb::Key::NumPad1 => KeyCode::KeyPad1,
        minifb::Key::NumPad2 => KeyCode::KeyPad2,
        minifb::Key::NumPad3 => KeyCode::KeyPad3,
        minifb::Key::NumPad4 => KeyCode::KeyPad4,
        minifb::Key::NumPad5 => KeyCode::KeyPad5,
        minifb::Key::NumPad6 => KeyCode::KeyPad6,
        minifb::Key::NumPad7 => KeyCode::KeyPad7,
        minifb::Key::NumPad8 => KeyCode::KeyPad8,
        minifb::Key::NumPad9 => KeyCode::KeyPad9,

        // Function Keys
        minifb::Key::F1 => KeyCode::F1,
        minifb::Key::F2 => KeyCode::F2,
        minifb::Key::F3 => KeyCode::F3,
        minifb::Key::F4 => KeyCode::F4,
        minifb::Key::F5 => KeyCode::F5,
        minifb::Key::F6 => KeyCode::F6,
        minifb::Key::F7 => KeyCode::F7,
        minifb::Key::F8 => KeyCode::F8,
        minifb::Key::F9 => KeyCode::F9,
        minifb::Key::F10 => KeyCode::F10,
        minifb::Key::F11 => KeyCode::F11,
        minifb::Key::F12 => KeyCode::F12,
        minifb::Key::F13 => KeyCode::F13,
        minifb::Key::F14 => KeyCode::F14,
        minifb::Key::F15 => KeyCode::F15,

        // Modifiers
        minifb::Key::LeftShift => KeyCode::LeftShift,
        minifb::Key::RightShift => KeyCode::RightShift,
        minifb::Key::LeftCtrl => KeyCode::LeftControl,
        minifb::Key::RightCtrl => KeyCode::RightControl,
        minifb::Key::LeftAlt => KeyCode::LeftAlt,
        minifb::Key::RightAlt => KeyCode::RightAlt,
        minifb::Key::LeftSuper => KeyCode::LeftSuper,
        minifb::Key::RightSuper => KeyCode::RightSuper,

        // Symbols
        minifb::Key::Space => KeyCode::Space,
        minifb::Key::Enter => KeyCode::Enter,
        minifb::Key::Escape => KeyCode::Escape,
        minifb::Key::Backspace => KeyCode::Backspace,
        minifb::Key::Tab => KeyCode::Tab,

        // Arrows
        minifb::Key::Up => KeyCode::UpArrow,
        minifb::Key::Down => KeyCode::DownArrow,
        minifb::Key::Left => KeyCode::LeftArrow,
        minifb::Key::Right => KeyCode::RightArrow,

        // Extras
        minifb::Key::Comma => KeyCode::Comma,
        minifb::Key::Period => KeyCode::Period,
        minifb::Key::Minus => KeyCode::Minus,
        minifb::Key::Equal => KeyCode::Equal,
        minifb::Key::LeftBracket => KeyCode::LeftBracket,
        minifb::Key::RightBracket => KeyCode::RightBracket,
        minifb::Key::Backslash => KeyCode::Backslash,
        minifb::Key::Semicolon => KeyCode::Semicolon,
        minifb::Key::Apostrophe => KeyCode::Quote,

        // Other
        minifb::Key::ScrollLock => KeyCode::ScrollLock,
        minifb::Key::CapsLock => KeyCode::CapsLock,
        minifb::Key::NumLock => KeyCode::NumLock,
        minifb::Key::Insert => KeyCode::Insert,
        minifb::Key::Delete => KeyCode::Delete,
        minifb::Key::Home => KeyCode::Home,
        minifb::Key::End => KeyCode::End,
        minifb::Key::PageUp => KeyCode::PageUp,
        minifb::Key::PageDown => KeyCode::PageDown,
        minifb::Key::Slash => KeyCode::Slash,
        minifb::Key::Menu => KeyCode::Menu,
        minifb::Key::Pause => KeyCode::Pause,

        // Unknown or unmapped
        minifb::Key::Unknown | minifb::Key::Count => KeyCode::Unknown,
        minifb::Key::Backquote => KeyCode::Grave,
        minifb::Key::NumPadDot => KeyCode::KeyPadDecimal,
        minifb::Key::NumPadSlash => KeyCode::KeyPadDivide,
        minifb::Key::NumPadAsterisk => KeyCode::KeyPadMultiply,
        minifb::Key::NumPadMinus => KeyCode::KeyPadSubtract,
        minifb::Key::NumPadPlus => KeyCode::KeyPadAdd,
        minifb::Key::NumPadEnter => KeyCode::KeyPadEnter,
        // This minifb key is a statistic, not an actual key. Wtf is it doing inside the enum variants???
    }
}
