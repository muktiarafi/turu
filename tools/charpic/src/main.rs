//! `charpic`: an AQW character's page as pictures, on Turu's Ruffle (ruffle-aqw) without a
//! window: the character card the page shows, and the Profile Pic the page makes.
//!
//! The page (`https://account.aq.com/CharPage?id=<name>`) embeds `characterB.swf` with the
//! character's gear in its FlashVars. In the SWF, "Profile Pic" opens an editor and "Make Image!"
//! draws the portrait into a `BitmapData`, encodes it with the SWF's own PNG encoder and hands it
//! to the page as base64 through `ExternalInterface.call("saveImage", ...)`. This program loads
//! the page, plays the SWF offscreen with the page's FlashVars, clicks those two buttons and
//! writes the PNG `saveImage` receives. Before the clicks, it saves the card itself (name, class,
//! level, gear, the character in its pose) as drawn, at `--scale` times the page's size.
//!
//! ```text
//! cargo run --release -p turu_charpic -- Questwalker [-o Profile-Questwalker.png]
//!     [--card CharPage-Questwalker.png] [--scale 2]
//! ```

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use ruffle_core::backend::navigator::{OwnedFuture, SocketMode};
use ruffle_core::backend::ui::NullUiBackend;
use ruffle_core::events::{MouseButton, PlayerEvent};
use ruffle_core::external::{ExternalInterfaceProvider, Value};
use ruffle_core::loader::Error as LoadError;
use ruffle_core::{FloatDuration, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::navigator::{
    ExternalNavigatorBackend, FutureSpawner, NavigatorInterface,
};
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render_wgpu::backend::{
    WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device,
};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::TextureTarget;
use ruffle_render_wgpu::wgpu;
use std::any::Any;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker};
use std::time::{Duration, Instant};
use url::Url;

/// The character page's stage, as its `<embed>` sizes it.
const STAGE: (u32, u32) = (715, 455);

/// "Profile Pic" on the character screen, in stage pixels (measured on the page: it doesn't move).
const PROFILE_PIC_BUTTON: (f64, f64) = (27.0, 429.0);

/// "Make Image!" in the Profile Pic editor, in stage pixels.
const MAKE_IMAGE_BUTTON: (f64, f64) = (163.0, 402.0);

/// The page sends browsers here; the SWF's gear loads go to its host.
const PAGE: &str = "https://account.aq.com/CharPage?id=";

/// The page is fetched as a browser would fetch it.
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) \
                          Chrome/131.0 Safari/537.36";

/// How long the gear may take to load, and the encoder to finish, before giving up.
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
const ENCODE_TIMEOUT: Duration = Duration::from_secs(120);

/// The gear is in once no download is left for this long.
const SETTLED: Duration = Duration::from_secs(2);

struct Args {
    character: String,
    /// The Profile Pic.
    out: PathBuf,
    /// The character card.
    card: PathBuf,
    /// The card's size, in times the page's.
    scale: f64,
}

fn parse_args() -> Result<Args> {
    let usage = "usage: charpic <character name or CharPage URL> [-o profile.png] [--card card.png] \
                 [--scale 2]";
    let mut character = None;
    let mut out = None;
    let mut card = None;
    let mut scale = 2.0;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--out" => out = Some(PathBuf::from(args.next().context(usage)?)),
            "--card" | "--frame" => card = Some(PathBuf::from(args.next().context(usage)?)),
            "--scale" => {
                scale = args.next().context(usage)?.parse().context(usage)?;
                if !(0.25..=8.0).contains(&scale) {
                    bail!("--scale must be between 0.25 and 8");
                }
            }
            "-h" | "--help" => {
                println!("{usage}");
                std::process::exit(0);
            }
            _ if character.is_none() && !arg.starts_with('-') => character = Some(arg),
            _ => bail!("{usage}"),
        }
    }
    let character = character.context(usage)?;
    let name = character
        .rsplit('=')
        .next()
        .unwrap_or(&character)
        .to_owned();
    let out = out.unwrap_or_else(|| PathBuf::from(format!("Profile-{name}.png")));
    let card = card.unwrap_or_else(|| PathBuf::from(format!("CharPage-{name}.png")));
    Ok(Args {
        character,
        out,
        card,
        scale,
    })
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let page_url = if args.character.starts_with("http") {
        Url::parse(&args.character)?
    } else {
        let mut url = Url::parse(PAGE)?;
        url.query_pairs_mut()
            .clear()
            .append_pair("id", &args.character);
        url
    };

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let html = runtime.block_on(fetch_page(&page_url))?;
    let embed =
        Embed::from_page(&html).with_context(|| format!("{page_url} shows no character"))?;
    eprintln!("{}: {} FlashVars", embed.movie, embed.flash_vars.len());

    let saved = Rc::new(RefCell::new(None));
    let tasks = Tasks::default();
    let player = build_player(&page_url, &embed, tasks.clone(), saved.clone(), args.scale)?;
    let run = Runner {
        player,
        tasks,
        scale: args.scale,
    };

    // The character and the gear it loads one after another.
    run.until(LOAD_TIMEOUT, "the character to load", |run| {
        run.settled_for() >= SETTLED
    })?;
    run.frames(15);
    let card = run.capture()?;
    card.save(&args.card)?;
    eprintln!(
        "wrote {} ({}x{})",
        args.card.display(),
        card.width(),
        card.height()
    );

    run.click(PROFILE_PIC_BUTTON);
    run.frames(30);
    run.click(MAKE_IMAGE_BUTTON);
    run.until(ENCODE_TIMEOUT, "saveImage", |_| saved.borrow().is_some())?;

    let base64 = saved.borrow_mut().take().unwrap_or_default();
    let png = base64::engine::general_purpose::STANDARD
        .decode(base64.trim())
        .context("saveImage's argument isn't base64")?;
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .context("saveImage's argument isn't a PNG")?;
    std::fs::write(&args.out, &png)?;
    eprintln!(
        "wrote {} ({}x{}, {} bytes)",
        args.out.display(),
        image.width(),
        image.height(),
        png.len()
    );
    Ok(())
}

async fn fetch_page(url: &Url) -> Result<String> {
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
    let response = client.get(url.clone()).send().await?.error_for_status()?;
    Ok(response.text().await?)
}

/// What the page's `<embed>` plays: the SWF's URL and its FlashVars.
struct Embed {
    movie: Url,
    flash_vars: Vec<(String, String)>,
}

impl Embed {
    fn from_page(html: &str) -> Option<Self> {
        let movie = attribute_after(html, "<embed", "src")?;
        let flash_vars = attribute_after(html, "<embed", "flashvars")?;
        let flash_vars = url::form_urlencoded::parse(flash_vars.as_bytes())
            .filter(|(name, _)| !name.is_empty())
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        Some(Self {
            movie: Url::parse(&movie).ok()?,
            flash_vars,
        })
    }
}

/// The HTML-unescaped value of `name="..."` in the first `tag` element.
fn attribute_after(html: &str, tag: &str, name: &str) -> Option<String> {
    let element = &html[html.find(tag)?..];
    let element = &element[..element.find('>')?];
    let start = element.find(&format!("{name}=\""))? + name.len() + 2;
    let value = &element[start..];
    let value = &value[..value.find('"')?];
    Some(
        value
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&#39;", "'")
            .replace("&amp;", "&"),
    )
}

fn build_player(
    page_url: &Url,
    embed: &Embed,
    tasks: Tasks,
    saved: Rc<RefCell<Option<String>>>,
    scale: f64,
) -> Result<Arc<Mutex<Player>>> {
    let size = (
        (f64::from(STAGE.0) * scale).round() as u32,
        (f64::from(STAGE.1) * scale).round() as u32,
    );
    let instance =
        create_wgpu_instance(wgpu::Backends::all(), wgpu::BackendOptions::default(), None);
    let (adapter, device, queue) = tokio::runtime::Handle::current()
        .block_on(request_adapter_and_device(
            wgpu::Backends::all(),
            &instance,
            None,
            wgpu::PowerPreference::HighPerformance,
        ))
        .map_err(|e| anyhow!("no GPU: {e}"))?;
    let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));
    let target = TextureTarget::new(&descriptors.device, size).map_err(|e| anyhow!("{e}"))?;
    let renderer = WgpuRenderBackend::new(descriptors, target).map_err(|e| anyhow!("{e}"))?;

    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: embed.movie.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        embed.movie.clone(),
        Some(page_url.clone()),
        None,
        tasks,
        None,
        false,
        Default::default(),
        SocketMode::Deny,
        Rc::new(content),
        NoBrowser,
    );
    let player = PlayerBuilder::new()
        .with_navigator(navigator)
        .with_renderer(renderer)
        .with_ui(NullUiBackend::new())
        .with_external_interface(Box::new(Page { saved }))
        .with_autoplay(true)
        .with_viewport_dimensions(size.0, size.1, scale)
        // The SWF's PNG encoder walks every pixel in ActionScript in one go.
        .with_max_execution_duration(ENCODE_TIMEOUT)
        .with_page_url(Some(page_url.to_string()))
        .build();
    player
        .lock()
        .expect("the player was just built")
        .fetch_root_movie(
            embed.movie.to_string(),
            embed.flash_vars.clone(),
            Box::new(|_| {}),
        );
    Ok(player)
}

/// Plays the player in real time (its downloads need it) and runs its futures between frames.
struct Runner {
    player: Arc<Mutex<Player>>,
    tasks: Tasks,
    /// Viewport pixels per stage pixel.
    scale: f64,
}

impl Runner {
    fn frame(&self) {
        let started = Instant::now();
        let dt = {
            let mut player = self.player.lock().expect("the player isn't poisoned");
            let dt = 1000.0 / player.frame_rate();
            player.tick(FloatDuration::from_millis(dt));
            dt
        };
        self.tasks.poll();
        let left = Duration::from_secs_f64(dt / 1000.0).saturating_sub(started.elapsed());
        std::thread::sleep(left);
    }

    fn frames(&self, count: u32) {
        for _ in 0..count {
            self.frame();
        }
    }

    fn until(
        &self,
        timeout: Duration,
        what: &str,
        mut done: impl FnMut(&Self) -> bool,
    ) -> Result<()> {
        let started = Instant::now();
        while !done(self) {
            if started.elapsed() > timeout {
                bail!("gave up waiting for {what} after {timeout:?}");
            }
            self.frame();
        }
        Ok(())
    }

    fn settled_for(&self) -> Duration {
        self.tasks.idle_for()
    }

    /// Moves the mouse onto `at`, presses and lets go, a frame apart, as a hand would.
    fn click(&self, (x, y): (f64, f64)) {
        let (x, y) = (x * self.scale, y * self.scale);
        let events = [
            PlayerEvent::MouseMove { x, y },
            PlayerEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                index: None,
            },
            PlayerEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            },
        ];
        for event in events {
            self.player
                .lock()
                .expect("the player isn't poisoned")
                .handle_event(event);
            self.frames(2);
        }
    }

    /// The stage as drawn now, straight alpha.
    fn capture(&self) -> Result<image::RgbaImage> {
        let mut player = self.player.lock().expect("the player isn't poisoned");
        player.render();
        <dyn Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(player.renderer_mut())
            .context("the renderer isn't offscreen")?
            .capture_frame()
            .context("no frame drawn")
    }
}

/// The player's futures (downloads), polled on the main thread after each frame: they lock the
/// player, so never while it's held. The network itself runs on the tokio runtime's threads.
#[derive(Clone, Default)]
struct Tasks(Rc<RefCell<TaskState>>);

struct TaskState {
    pending: Vec<OwnedFuture<(), LoadError>>,
    idle_since: Instant,
}

impl Default for TaskState {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            idle_since: Instant::now(),
        }
    }
}

impl Tasks {
    fn poll(&self) {
        let mut pending = std::mem::take(&mut self.0.borrow_mut().pending);
        let mut cx = TaskContext::from_waker(Waker::noop());
        pending.retain_mut(|future| match future.as_mut().poll(&mut cx) {
            Poll::Pending => true,
            Poll::Ready(Err(e)) => {
                eprintln!("load error: {e}");
                false
            }
            Poll::Ready(Ok(())) => false,
        });
        let mut state = self.0.borrow_mut();
        // Futures the finished ones started were spawned meanwhile.
        state.pending.append(&mut pending);
        if !state.pending.is_empty() {
            state.idle_since = Instant::now();
        }
    }

    fn idle_for(&self) -> Duration {
        let state = self.0.borrow();
        if state.pending.is_empty() {
            state.idle_since.elapsed()
        } else {
            Duration::ZERO
        }
    }
}

impl FutureSpawner<LoadError> for Tasks {
    fn spawn(&self, future: OwnedFuture<(), LoadError>) {
        let mut state = self.0.borrow_mut();
        state.pending.push(future);
        state.idle_since = Instant::now();
    }
}

/// The page around the SWF: it hears `saveImage(base64)` and answers nothing else.
struct Page {
    saved: Rc<RefCell<Option<String>>>,
}

impl ExternalInterfaceProvider for Page {
    fn call_method(
        &self,
        _context: &mut ruffle_core::context::UpdateContext<'_>,
        name: &str,
        args: &[Value],
    ) -> Value {
        match (name, args.first()) {
            ("saveImage", Some(Value::String(base64))) => {
                *self.saved.borrow_mut() = Some(base64.clone());
            }
            _ => eprintln!("the SWF called {name} ({} arguments)", args.len()),
        }
        Value::Undefined
    }

    fn on_callback_available(&self, _name: &str) {}

    fn get_id(&self) -> Option<String> {
        None
    }
}

/// No browser: the SWF's links aren't opened, files and sockets refused.
#[derive(Clone)]
struct NoBrowser;

impl NavigatorInterface for NoBrowser {
    fn navigate_to_website(&self, url: Url) {
        eprintln!("the SWF opens {url} (not followed)");
    }

    async fn open_file(&self, path: &Path) -> std::io::Result<std::fs::File> {
        std::fs::File::open(path)
    }

    async fn confirm_socket(&self, _host: &str, _port: u16) -> bool {
        false
    }
}
