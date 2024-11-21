use std::{
    io::{self, Write},
    sync::mpsc::TryRecvError,
    thread::{self, sleep},
    time::{Duration, Instant},
};

use std::sync::mpsc::{self, Receiver};

use windows_capture::{
    capture::{Context, GraphicsCaptureApiHandler},
    encoder::{AudioSettingsBuilder, ContainerSettingsBuilder, VideoEncoder, VideoSettingsBuilder},
    frame::Frame,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{ColorFormat, CursorCaptureSettings, DrawBorderSettings, Settings},
};

#[derive(Debug)]
struct CaptureContext {
    name: String,
    width: u32,
    height: u32,
    rx: Receiver<()>,
}
// Handles capture events.
struct Capture {
    // The video encoder that will be used to encode the frames.
    encoder: Option<VideoEncoder>,
    // To measure the time the capture has been running
    start: Instant,
    rx: Receiver<()>,
}

impl GraphicsCaptureApiHandler for Capture {
    // The type of flags used to get the values from the settings.
    type Flags = CaptureContext;

    // The type of error that can be returned from `CaptureControl` and `start` functions.
    type Error = Box<dyn std::error::Error + Send + Sync>;

    // Function that will be called to create a new instance. The flags can be passed from settings.
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        println!("Created with Flags: {:?}", ctx.flags);

        let encoder = VideoEncoder::new(
            VideoSettingsBuilder::new(ctx.flags.width, ctx.flags.height)
                .sub_type(windows_capture::encoder::VideoSettingsSubType::H264),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            ctx.flags.name,
        )?;

        Ok(Self {
            encoder: Some(encoder),
            start: Instant::now(),
            rx: ctx.flags.rx,
        })
    }

    // Called every time a new frame is available.
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        print!(
            "\rRecording for: {} seconds",
            self.start.elapsed().as_secs()
        );
        io::stdout().flush()?;

        // Send the frame to the video encoder
        self.encoder.as_mut().unwrap().send_frame(frame)?;

        // check if rx is disconnected
        match self.rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                // Finish the encoder and save the video.
                self.encoder.take().unwrap().finish()?;

                capture_control.stop();

                println!();
            }
            Err(TryRecvError::Empty) => (),
        };

        Ok(())
    }

    // Optional handler called when the capture item (usually a window) closes.
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        println!("Capture session ended");

        Ok(())
    }
}

fn main() {
    // Gets the foreground window, refer to the docs for other capture items
    let primary_monitor = Monitor::primary().expect("There is no primary monitor");

    let (tx, rx) = mpsc::channel::<()>();
    let settings = Settings::new(
        // Item to capture
        primary_monitor,
        // Capture cursor settings
        CursorCaptureSettings::WithCursor,
        // Draw border settings
        DrawBorderSettings::WithoutBorder,
        // The desired color format for the captured frame.
        ColorFormat::Bgra8,
        // Additional flags for the capture settings that will be passed to user defined `new` function.
        CaptureContext {
            name: "video.mp4".to_string(),
            width: primary_monitor.width().unwrap(),
            height: primary_monitor.height().unwrap(),
            rx,
        },
    );

    let recorder_thread = thread::spawn(move || {
        // Starts the capture and takes control of the current thread.
        // The errors from handler trait will end up here
        match Capture::start(settings) {
            Ok(_) => println!("Capture ended successfully"),
            Err(e) => eprintln!("Error: {}", e),
        }
    });

    sleep(Duration::from_secs(60);
    tx.send(()).unwrap();

    let _ = recorder_thread.join();
}
