//! Turu: a client's pictures as GPU surfaces the shell draws from, with no copy through the CPU
//! (docs/TAB_PROCESSES_PROPOSAL.md, M5; macOS only, the shared textures of the proposal's
//! "Desktop"). Checked first by `examples/iosurface_share.rs` (M2).
//!
//! The shell checks a Mach service in with the bootstrap server ([`listen`]) and names it to its
//! clients in `TURU_SHELL_SURFACES`. A client makes each surface it draws into as an IOSurface,
//! opens it in wgpu as a Metal texture, and sends the surface's Mach port to that service with its
//! process id and the surface's id ([`Sender`], a `SharedSurfaces` for the client's screen). The
//! shell opens the same surface as a texture of its own GPU device ([`Surface::texture`]).
//! Which surface holds the newest picture, and when the shell is done with one, go over the
//! socket (`ToShell::Surface`, `ToClient::Release`).

use mach2::bootstrap::{bootstrap_check_in, bootstrap_look_up, bootstrap_port};
use mach2::kern_return::KERN_SUCCESS;
use mach2::mach_port::mach_port_deallocate;
use mach2::message::{
    MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MOVE_SEND, MACH_MSGH_BITS, MACH_MSGH_BITS_COMPLEX, MACH_RCV_MSG,
    MACH_SEND_MSG, MACH_SEND_TIMEOUT, mach_msg, mach_msg_body_t, mach_msg_header_t, mach_msg_port_descriptor_t,
    mach_msg_trailer_t,
};
use mach2::port::{MACH_PORT_NULL, mach_port_t};
use mach2::traps::mach_task_self;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_io_surface::{
    IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight, kIOSurfacePixelFormat, kIOSurfaceWidth,
};
use objc2_metal::{MTLDevice, MTLPixelFormat, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage};
use std::ffi::CString;
use turu_frontend::client_screen::SharedSurfaces;
use wgpu::hal::api::Metal;

/// 'BGRA': the pictures' format (`wgpu::TextureFormat::Bgra8Unorm`).
const PIXEL_FORMAT: i32 = 0x4247_5241;
/// "turu": a surface's message.
const MESSAGE_ID: i32 = 0x7475_7275;

/// A surface's port, with the sender's process id and the surface's id (low, high).
#[repr(C)]
struct PortMessage {
    header: mach_msg_header_t,
    body: mach_msg_body_t,
    port: mach_msg_port_descriptor_t,
    data: [u32; 3],
}

#[repr(C)]
struct ReceivedMessage {
    message: PortMessage,
    trailer: mach_msg_trailer_t,
}

/// An IOSurface from a client.
pub struct Surface(CFRetained<IOSurfaceRef>);

// SAFETY: an IOSurface may be used from any thread (its reference count is atomic, and it's only
// read here, through Metal).
unsafe impl Send for Surface {}

impl Surface {
    pub fn size(&self) -> (u32, u32) {
        (self.0.width() as u32, self.0.height() as u32)
    }

    /// The surface as a texture of `device` the shell can sample.
    pub fn texture(&self, device: &wgpu::Device) -> Option<wgpu::Texture> {
        texture(device, &self.0, wgpu::TextureUsages::TEXTURE_BINDING)
    }
}

/// Checks in the shell's Mach service and receives the clients' surfaces on a thread of its own,
/// handing each to `got` with the client's process id and the surface's id. The service's name,
/// for the clients; `None` if it couldn't be checked in (the clients then send pixels).
pub fn listen(got: impl Fn(u32, u64, Surface) + Send + 'static) -> Option<String> {
    let name = format!("com.muktiarafi.turu.shell.{}", std::process::id());
    let c_name = CString::new(name.clone()).ok()?;
    let mut receive: mach_port_t = MACH_PORT_NULL;
    // SAFETY: a C string and an out-pointer.
    let kr = unsafe { bootstrap_check_in(bootstrap_port, c_name.as_ptr(), &mut receive) };
    if kr != KERN_SUCCESS {
        tracing::warn!("Shell: couldn't check in {name} ({kr}): clients will send pixels");
        return None;
    }
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Surfaces".into())
        .spawn(move || {
            loop {
                // SAFETY: a zeroed buffer as large as the message with its trailer.
                let mut received: ReceivedMessage = unsafe { std::mem::zeroed() };
                // SAFETY: receives into that buffer, its size given.
                let kr = unsafe {
                    mach_msg(
                        &mut received.message.header,
                        MACH_RCV_MSG,
                        0,
                        size_of::<ReceivedMessage>() as u32,
                        receive,
                        0,
                        MACH_PORT_NULL,
                    )
                };
                if kr != KERN_SUCCESS {
                    tracing::warn!("Shell: receiving a surface failed ({kr:#x})");
                    continue;
                }
                let message = &received.message;
                let complex = message.header.msgh_bits & MACH_MSGH_BITS_COMPLEX != 0;
                if message.header.msgh_id != MESSAGE_ID || !complex || message.body.msgh_descriptor_count != 1 {
                    tracing::warn!("Shell: a Mach message that isn't a surface; ignored");
                    continue;
                }
                let port = message.port.name;
                let surface = IOSurfaceRef::lookup_from_mach_port(port);
                // SAFETY: the send right the message moved to this task, no longer needed (the
                // lookup holds the surface).
                unsafe { mach_port_deallocate(mach_task_self(), port) };
                let [pid, low, high] = message.data;
                match surface {
                    Some(surface) => got(pid, u64::from(low) | u64::from(high) << 32, Surface(surface)),
                    None => tracing::warn!("Shell: client {pid}'s surface couldn't be opened"),
                }
            }
        });
    match spawned {
        Ok(_) => Some(name),
        Err(e) => {
            tracing::warn!("Shell: couldn't start receiving surfaces: {e}");
            None
        }
    }
}

/// A client's surfaces, made for its screen and sent to the shell's service.
pub struct Sender {
    shell: mach_port_t,
    next_id: u64,
}

// SAFETY: a send right is a number in this task's port namespace, usable from any thread.
unsafe impl Send for Sender {}

impl Sender {
    /// Finds the shell's service by `name` ([`listen`]'s).
    pub fn connect(name: &str) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut shell: mach_port_t = MACH_PORT_NULL;
        // SAFETY: a C string and an out-pointer.
        let kr = unsafe { bootstrap_look_up(bootstrap_port, c_name.as_ptr(), &mut shell) };
        if kr != KERN_SUCCESS {
            tracing::warn!("Couldn't find the shell's surfaces at {name} ({kr}): sending pixels");
            return None;
        }
        Some(Self { shell, next_id: 1 })
    }

    fn send(&self, surface: &IOSurfaceRef, id: u64) -> bool {
        let port = surface.create_mach_port();
        let mut message = PortMessage {
            header: mach_msg_header_t {
                msgh_bits: MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0) | MACH_MSGH_BITS_COMPLEX,
                msgh_size: size_of::<PortMessage>() as u32,
                msgh_remote_port: self.shell,
                msgh_local_port: MACH_PORT_NULL,
                msgh_voucher_port: MACH_PORT_NULL,
                msgh_id: MESSAGE_ID,
            },
            body: mach_msg_body_t {
                msgh_descriptor_count: 1,
            },
            port: mach_msg_port_descriptor_t::new(port, MACH_MSG_TYPE_MOVE_SEND),
            data: [std::process::id(), id as u32, (id >> 32) as u32],
        };
        // SAFETY: a complete message to a send right; a shell that stopped receiving doesn't hold
        // the engine for more than a second.
        let kr = unsafe {
            mach_msg(
                &mut message.header,
                MACH_SEND_MSG | MACH_SEND_TIMEOUT,
                size_of::<PortMessage>() as u32,
                0,
                MACH_PORT_NULL,
                1000,
                MACH_PORT_NULL,
            )
        };
        if kr != KERN_SUCCESS {
            // Not moved: this task still holds the right.
            // SAFETY: the send right `create_mach_port` made.
            unsafe { mach_port_deallocate(mach_task_self(), port) };
            tracing::warn!("Couldn't hand a surface to the shell ({kr:#x})");
            return false;
        }
        true
    }
}

impl SharedSurfaces for Sender {
    fn make(&mut self, device: &wgpu::Device, (width, height): (u32, u32)) -> Option<(u64, wgpu::Texture)> {
        let surface = new_surface(width, height)?;
        let texture = texture(device, &surface, wgpu::TextureUsages::RENDER_ATTACHMENT)?;
        let id = self.next_id;
        self.next_id += 1;
        // The Metal texture keeps the surface alive on this side; the shell's lookup on its own.
        self.send(&surface, id).then_some((id, texture))
    }
}

fn new_surface(width: u32, height: u32) -> Option<CFRetained<IOSurfaceRef>> {
    let number = |n: i32| CFNumber::new_i32(n);
    let (w, h, bpe, format) = (
        number(width as i32),
        number(height as i32),
        number(4),
        number(PIXEL_FORMAT),
    );
    // SAFETY: the keys are IOSurface's constants.
    let keys: [&CFString; 4] = unsafe {
        [
            kIOSurfaceWidth,
            kIOSurfaceHeight,
            kIOSurfaceBytesPerElement,
            kIOSurfacePixelFormat,
        ]
    };
    let values: [&CFType; 4] = [&w, &h, &bpe, &format];
    let properties = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    // SAFETY: a dictionary of IOSurface's property keys and numbers.
    let surface = unsafe { IOSurfaceRef::new(properties.as_opaque()) };
    if surface.is_none() {
        tracing::warn!("Couldn't make a {width}x{height} IOSurface");
    }
    surface
}

/// `surface` as a BGRA texture of `device` (a Metal one), for `usage`.
fn texture(device: &wgpu::Device, surface: &IOSurfaceRef, usage: wgpu::TextureUsages) -> Option<wgpu::Texture> {
    let (width, height) = (surface.width() as u32, surface.height() as u32);
    // SAFETY: only Metal's device is used, if it is one.
    let hal_device = unsafe { device.as_hal::<Metal>() }?;
    let raw_device = hal_device.raw_device();
    // SAFETY: a plain 2D descriptor.
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::BGRA8Unorm,
            width as usize,
            height as usize,
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
    // Shared where the GPU and CPU share memory (Apple's chips), else managed (Intel Macs).
    descriptor.setStorageMode(if raw_device.hasUnifiedMemory() {
        MTLStorageMode::Shared
    } else {
        MTLStorageMode::Managed
    });
    let Some(raw): Option<Retained<ProtocolObject<dyn MTLTexture>>> =
        raw_device.newTextureWithDescriptor_iosurface_plane(&descriptor, surface, 0)
    else {
        tracing::warn!("Metal couldn't open a {width}x{height} IOSurface as a texture");
        return None;
    };
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    // SAFETY: a 2D BGRA texture of that size; the hal texture owns the Metal one.
    let hal_texture = unsafe {
        wgpu::hal::metal::Device::texture_from_raw(
            raw,
            wgpu::TextureFormat::Bgra8Unorm,
            objc2_metal::MTLTextureType::Type2D,
            1,
            1,
            wgpu::hal::CopyExtent {
                width,
                height,
                depth: 1,
            },
            None,
        )
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("Turu shared picture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage,
        view_formats: &[],
    };
    let initial = if usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) {
        wgpu::TextureUses::COLOR_TARGET
    } else {
        wgpu::TextureUses::RESOURCE
    };
    // SAFETY: the hal texture matches `desc`, and the surface's contents are kept.
    Some(unsafe { device.create_texture_from_hal::<Metal>(hal_texture, &desc, initial) })
}
