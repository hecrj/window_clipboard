use window_clipboard::Clipboard;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let clipboard = Clipboard::new(&window).expect("Create clipboard");

    let mut i = 0;

    event_loop.run(move |event, _, control_flow| match event {
        Event::MainEventsCleared => {
            println!(
                "write: {:?}",
                clipboard.write(format!("hello world {}", i))
            );
            i += 1;

            println!("read: {:?}", clipboard.read());
        }
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => *control_flow = ControlFlow::Exit,
        _ => *control_flow = ControlFlow::Wait,
    });
}
