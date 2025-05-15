use std::mem::MaybeUninit;
use thiserror::Error;
use std::hash::Hash;
use ahash::{HashSet, HashSetExt};
use glam::{vec2, Vec2};
use winit::event::{DeviceEvent, ElementState, MouseButton, RawKeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub trait Button: Copy + Send + Sync + Hash + Eq + 'static  {}

impl<T: Copy + Send + Sync + Hash + Eq + 'static> Button for T {}

#[derive(Debug)]
pub struct ButtonInput<T> {
    pressed: HashSet<T>,

    just_pressed: HashSet<T>,
    just_released: HashSet<T>
}


impl<T: Button> ButtonInput<T> {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),

            just_pressed: HashSet::new(),
            just_released: HashSet::new()
        }
    }

    /// Registers a press for the given `input`.
    pub fn press(&mut self, input: T) {
        // Returns `true` if the `input` wasn't pressed.
        if self.pressed.insert(input) {
            self.just_pressed.insert(input);
        }
    }

    pub fn pressed(&self, input: T) -> bool {
        self.pressed.contains(&input)
    }

    pub fn just_pressed(&self, input: T) -> bool {
        self.just_pressed.contains(&input)
    }

    pub fn release(&mut self, input: T) {
        // Returns `true` if the `input` was pressed.
        if self.pressed.remove(&input) {
            self.just_released.insert(input);
        }
    }


    /// Registers a release for all currently pressed inputs.
    pub fn release_all(&mut self) {
        // Move all items from pressed into just_released
        self.just_released.extend(self.pressed.drain());
    }

    /// Clears the `pressed`, `just_pressed`, and `just_released` data for every input.
    ///
    /// See also [`ButtonInput::clear`] for simulating elapsed time steps.
    pub fn reset_all(&mut self) {
        self.pressed.clear();
        self.just_pressed.clear();
        self.just_released.clear();
    }


    /// Clears the `just pressed` and `just released` data for every input.
    ///
    /// See also [`ButtonInput::reset_all`] for a full reset.
    pub fn clear(&mut self) {
        self.just_pressed.clear();
        self.just_released.clear();
    }
}


// this uses implicit discriminants
// https://doc.rust-lang.org/reference/items/enumerations.html#r-items.enum.discriminant.implicit

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
enum KeybindLen {
    _1 = 1,
    _2,
    _3,
    _4,
    _5,
    _6,
    _7,
    _8,
}

#[derive(Debug, Clone)]
pub struct Keybinding<T: Copy> {
    len: KeybindLen,
    keys: [MaybeUninit<T>; 8]
}


impl<T: Copy> Keybinding<T> {
    const fn as_slice(&self) -> &[T] {
        let len = self.len as usize;
        unsafe {
            std::slice::from_raw_parts(
                self.keys.as_ptr().cast::<T>(),
                len
            )
        }
    }
}

#[derive(Debug, Error)]
#[error("Invalid length {0}, expected a length between 1 and 8")]
pub struct InvalidLength(usize);


impl<T: Button> Keybinding<T> {
    pub const fn from_slice(slice: &[T]) -> Result<Self, InvalidLength> {
        match slice.len() {
            len @ 1..8 => {
                let keybind_len = unsafe { std::mem::transmute::<u8, KeybindLen>(len as u8) };
                let mut keys = [const { MaybeUninit::uninit() }; 8];
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        slice.as_ptr(),
                        keys.as_mut_ptr().cast::<T>(),
                        len
                    )
                }

                Ok(Keybinding {
                    len: keybind_len,
                    keys
                })
            }

            invalid => Err(InvalidLength(invalid))
        }
    }

    pub fn keys(&self) -> impl Iterator<Item=T> {
        self.as_slice().iter().copied()
    }

    fn held_down(&self, inputs: &ButtonInput<T>) -> bool {
        self.keys().all(move |key| inputs.pressed(key))
    }

    fn triggered(&self, inputs: &ButtonInput<T>) -> bool {
        if let [single] = *self.as_slice() {
            return inputs.just_pressed(single)
        }

        self.held_down(inputs)
            && self.keys().any(move |key| inputs.just_pressed(key))
    }
}

#[derive(Debug)]
pub struct KeyMap<T: Copy> {
    key_map: [Keybinding<T>; ACTIONS_COUNT]
}

impl<T: Copy> KeyMap<T> {
    fn get(&self, mapping: KeyMapping) -> &Keybinding<T> {
        &self.key_map[mapping as usize]
    }
}

mod sealed {
    use crate::controls::MouseAndKeyboardButton;

    pub trait Sealed {}

    impl Sealed for MouseAndKeyboardButton {}
}

pub trait DefaultActions: Copy + sealed::Sealed {
    fn default_actions() -> KeyMap<Self>;
}

impl<T: DefaultActions> Default for KeyMap<T> {
    fn default() -> Self {
        T::default_actions()
    }
}

macro_rules! define_key_mappings {
    (
        actions_count: $count: ident,
        enum $action_enum: ident {
        $($action:ident MKB { $($mouse_and_keyboard:expr),+ $(,)? }),+ $(,)?
    }) => {
        #[derive(Copy, Clone, Eq, PartialEq, Hash)]
        pub enum $action_enum {
            $($action),*
        }

        const $count: usize = <[$action_enum]>::len(&[ $($action_enum::$action),* ]);

        impl DefaultActions for MouseAndKeyboardButton {
            fn default_actions() -> KeyMap<Self> {
                // made in const
                let key_map = const {
                    let mut last_action = None;

                    $(
                    let action = $action_enum::$action as usize;

                    match last_action {
                        None => assert!(action == 0),
                        Some(last) => assert!(action == last + 1),
                    }

                    last_action = Some(action);
                    )+

                    let _ = last_action;

                    [
                        $(
                            match Keybinding::from_slice(&[$($mouse_and_keyboard),+]) {
                                Ok(binding) => binding,
                                Err(_) => panic!(
                                    concat!("Mouse and keyboard binding for ", stringify!($action), "is too long")
                                )
                            }
                        ),+
                    ]
                };

                KeyMap { key_map }
            }
        }
    };
}


// FIXME support other methods of input
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[expect(dead_code, reason = "mouse controls soon")]
pub enum MouseAndKeyboardButton {
    Mouse(MouseButton),
    Keyboard(KeyCode)
}

#[expect(unused_macros, reason = "mouse controls soon")]
macro_rules! mouse {
    ($mouse_button: ident) => {
        MouseAndKeyboardButton::Mouse(MouseButton::$mouse_button)
    };
}

macro_rules! key {
    ($key: ident) => {
        MouseAndKeyboardButton::Keyboard(KeyCode::$key)
    };
}


define_key_mappings! {
    actions_count: ACTIONS_COUNT,
    enum KeyMapping {
        WalkForwards MKB { key!(KeyW) },
        WalkBackwards MKB { key!(KeyS) },
        WalkRight MKB { key!(KeyD) },
        WalkLeft MKB { key!(KeyA) },

        Jump MKB { key!(Space) },
        Sneak MKB { key!(ShiftLeft) },
        Sprint MKB { key!(ControlLeft) },



        MainMenu MKB { key!(Escape) },

        Exit MKB { key!(Escape), key!(Backspace) },

        Fullscreen MKB { key!(F11) },
    }
}

#[derive(Debug)]
pub struct Keybindings<T: Button> {
    inputs: ButtonInput<T>,
    map: KeyMap<T>,
}

impl<T: Button> Keybindings<T> {
    fn clear(&mut self) {
        self.inputs.clear()
    }

    fn held_down(&self, mapping: KeyMapping) -> bool {
        self.map.get(mapping).held_down(&self.inputs)
    }

    fn triggered(&self, mapping: KeyMapping) -> bool {
        self.map.get(mapping).triggered(&self.inputs)
    }
}


#[derive(Debug)]
struct MouseMotion {
    accumulated: Vec2
}

#[derive(Debug)]
struct MouseAndKeyboardInput {
    keys: Keybindings<MouseAndKeyboardButton>,
    mouse: MouseMotion
}

pub trait InputMethod {
    fn held_down(&self, mapping: KeyMapping) -> bool;

    fn triggered(&self, mapping: KeyMapping) -> bool;

    fn cursor_delta(&self) -> Vec2;
}


impl InputMethod for MouseAndKeyboardInput {
    fn held_down(&self, mapping: KeyMapping) -> bool {
        self.keys.held_down(mapping)
    }

    fn triggered(&self, mapping: KeyMapping) -> bool {
        self.keys.triggered(mapping)
    }

    fn cursor_delta(&self) -> Vec2 {
        self.mouse.accumulated
    }
}

#[derive(Debug)]
pub struct Controls {
    mkb: MouseAndKeyboardInput
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            mkb: MouseAndKeyboardInput {
                keys: Keybindings { 
                    inputs: ButtonInput::new(),
                    map: KeyMap::default()
                },
                mouse: MouseMotion { 
                    accumulated: Vec2::ZERO
                },
            }
        }
    }
}

impl Controls {
    pub fn new_frame(&mut self) {
        self.mkb.keys.clear();
        self.mkb.mouse.accumulated = Vec2::ZERO
    }

    pub fn lost_focus(&mut self) {
        let input = &mut self.mkb.keys.inputs;
        input.reset_all();
        input.release_all()
    }

    fn update_mkb_buttons(&mut self, code: MouseAndKeyboardButton, state: ElementState) {
        let inputs = &mut self.mkb.keys.inputs;
        match state {
            ElementState::Pressed =>  inputs.press(code),
            ElementState::Released => inputs.release(code),
        }
    }

    pub fn update(&mut self, window_event: &DeviceEvent) {
        match *window_event {
            DeviceEvent::Key(RawKeyEvent { physical_key: PhysicalKey::Code(code), state, .. }) =>
                {
                    self.update_mkb_buttons(MouseAndKeyboardButton::Keyboard(code), state)
                },
            DeviceEvent::MouseMotion { delta: (x, y) } => {
                self.mkb.mouse.accumulated += vec2(x as f32, y as f32);
            }
            _ => {}
        }
    }
}

impl InputMethod for Controls {
    fn held_down(&self, mapping: KeyMapping) -> bool {
        self.mkb.held_down(mapping)
    }

    fn triggered(&self, mapping: KeyMapping) -> bool {
        self.mkb.triggered(mapping)
    }

    fn cursor_delta(&self) -> Vec2 {
        self.mkb.cursor_delta()
    }
}