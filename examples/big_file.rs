use rand::distributions::{Alphanumeric, Distribution};
use window_clipboard::Clipboard;
use winit::{
    error::EventLoopError,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::WindowBuilder,
};

fn main() -> Result<(), EventLoopError> {
    let mut rng = rand::thread_rng();

    let data: String = Alphanumeric
        .sample_iter(&mut rng)
        .take(10_000_000)
        .map(char::from)
        .collect();

    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("Press G to start the test!")
        .build(&event_loop)
        .unwrap();

    let mut clipboard =
        unsafe { Clipboard::connect(&window) }.expect("Connect to clipboard");

    clipboard.write(data.clone()).unwrap();

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key: Key::Character(c),
                            state: ElementState::Released,
                            ..
                        },
                    ..
                },
            ..
        } if c == "G" => {
            let new_data = clipboard.read().expect("Read data");
            assert_eq!(data, new_data, "Data is equal");
            println!("Data copied successfully!");
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => elwt.exit(),
        _ => {}
    })
}
