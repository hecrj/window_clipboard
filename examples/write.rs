use window_clipboard::Clipboard;
use winit::{
    error::EventLoopError,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

fn main() -> Result<(), EventLoopError> {
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let mut clipboard =
        unsafe { Clipboard::connect(&window) }.expect("Connect to clipboard");

    clipboard
        .write(String::from("Hello, world!"))
        .expect("Write to clipboard");

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => elwt.exit(),
        _ => {}
    })
}
