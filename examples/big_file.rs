use rand::distributions::{Alphanumeric, Distribution};
use window_clipboard::Clipboard;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let mut rng = rand::thread_rng();

    let data: String = Alphanumeric
        .sample_iter(&mut rng)
        .take(10_000_000)
        .map(char::from)
        .collect();

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Press G to start the test!")
        .build(&event_loop)
        .unwrap();

    let mut clipboard =
        Clipboard::connect(&window).expect("Connect to clipboard");

    clipboard.write(data.clone()).unwrap();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event:
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::G),
                            state: ElementState::Released,
                            ..
                        },
                    ..
                },
            ..
        } => {
            let new_data = clipboard.read().expect("Read data");
            assert_eq!(data, new_data, "Data is equal");
            println!("Data copied successfully!");
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => *control_flow = ControlFlow::Exit,
        _ => *control_flow = ControlFlow::Wait,
    });
}
