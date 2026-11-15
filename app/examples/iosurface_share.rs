//! Turu: M2's shared-texture check (docs/TAB_PROCESSES_PROPOSAL.md): can a client process draw
//! into a GPU texture that the shell process then uses, with no copy through the CPU? On macOS
//! (Metal) through an IOSurface, handed over as a Mach port.
//!
//! The parent checks a unique name in with the bootstrap server, starts itself as the child with
//! that name, and waits for a Mach message. The child makes a BGRA IOSurface, opens it in wgpu as
//! a Metal texture, writes a known pattern into it on its GPU queue, and sends the surface's Mach
//! port to the parent. The parent opens the same surface as a wgpu texture of its own device,
//! copies it into a buffer on its GPU, and checks every pixel.
//!
//! Run: `cargo run -p turu --example iosurface_share --no-default-features --features
//! software_video,external_video,lzma,jpegxr,fontconfig-dlopen`

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS only: IOSurface is Apple's");
}

#[cfg(target_os = "macos")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("--child") => mac::child(&args[2]),
        _ => mac::parent(),
    };
    if let Err(e) = result {
        eprintln!("FAILED: {e}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use mach2::bootstrap::{bootstrap_check_in, bootstrap_look_up, bootstrap_port};
    use mach2::kern_return::KERN_SUCCESS;
    use mach2::message::{
        MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MOVE_SEND, MACH_MSGH_BITS, MACH_MSGH_BITS_COMPLEX, MACH_RCV_MSG,
        MACH_RCV_TIMEOUT, MACH_SEND_MSG, mach_msg, mach_msg_body_t, mach_msg_header_t, mach_msg_port_descriptor_t,
        mach_msg_trailer_t,
    };
    use mach2::port::{MACH_PORT_NULL, mach_port_t};
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType};
    use objc2_io_surface::{
        IOSurfaceLockOptions, IOSurfaceRef, kIOSurfaceBytesPerElement, kIOSurfaceHeight, kIOSurfacePixelFormat,
        kIOSurfaceWidth,
    };
    use objc2_metal::{MTLDevice, MTLPixelFormat, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage};
    use std::ffi::CString;
    use std::io::Read;
    use std::process::{Command, Stdio};
    use wgpu::hal::api::Metal;

    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 360;
    /// 'BGRA'.
    const PIXEL_FORMAT: i32 = 0x4247_5241;
    const MESSAGE_ID: i32 = 0x7475_7275;

    /// The pattern the child writes: BGRA bytes for (`x`, `y`).
    fn pattern(x: u32, y: u32) -> [u8; 4] {
        [(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 0xff]
    }

    /// A complex Mach message carrying one port.
    #[repr(C)]
    struct PortMessage {
        header: mach_msg_header_t,
        body: mach_msg_body_t,
        port: mach_msg_port_descriptor_t,
    }

    #[repr(C)]
    struct ReceivedMessage {
        message: PortMessage,
        trailer: mach_msg_trailer_t,
    }

    pub fn parent() -> Result<(), String> {
        let name = CString::new(format!("com.muktiarafi.turu.iosurface-share.{}", std::process::id())).unwrap();
        let mut receive: mach_port_t = MACH_PORT_NULL;
        // SAFETY: a C string and an out-pointer.
        let kr = unsafe { bootstrap_check_in(bootstrap_port, name.as_ptr(), &mut receive) };
        if kr != KERN_SUCCESS {
            return Err(format!("bootstrap_check_in failed ({kr})"));
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = Command::new(exe)
            .arg("--child")
            .arg(name.to_str().unwrap())
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        println!(
            "parent {}: child {} started; waiting for its surface",
            std::process::id(),
            child.id()
        );

        // SAFETY: a zeroed buffer as large as the message with its trailer.
        let mut received: ReceivedMessage = unsafe { std::mem::zeroed() };
        let kr = unsafe {
            mach_msg(
                &mut received.message.header,
                MACH_RCV_MSG | MACH_RCV_TIMEOUT,
                0,
                size_of::<ReceivedMessage>() as u32,
                receive,
                30_000,
                MACH_PORT_NULL,
            )
        };
        if kr != KERN_SUCCESS {
            let _ = child.kill();
            return Err(format!("no message from the child ({kr:#x})"));
        }
        if received.message.header.msgh_id != MESSAGE_ID || received.message.body.msgh_descriptor_count != 1 {
            let _ = child.kill();
            return Err("an unexpected message".into());
        }
        let port = received.message.port.name;
        let surface = IOSurfaceRef::lookup_from_mach_port(port).ok_or("IOSurfaceLookupFromMachPort failed")?;
        println!(
            "parent: got the child's surface ({}x{})",
            surface.width(),
            surface.height()
        );

        let (device, queue) = wgpu_device()?;
        let texture = surface_texture(&device, &surface, wgpu::TextureUsages::COPY_SRC)?;
        let pixels = read_back(&device, &queue, &texture)?;
        drop(child.stdin.take());
        let _ = child.wait();

        let mut wrong = 0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let at = ((y * WIDTH + x) * 4) as usize;
                if pixels[at..at + 4] != pattern(x, y) {
                    wrong += 1;
                }
            }
        }
        let sample = |x: u32, y: u32| {
            let at = ((y * WIDTH + x) * 4) as usize;
            format!("{:?}", &pixels[at..at + 4])
        };
        println!(
            "parent: read on its own GPU: (0,0) {} (321,123) {} (639,359) {}",
            sample(0, 0),
            sample(321, 123),
            sample(639, 359)
        );
        if wrong == 0 {
            println!(
                "OK: all {} pixels the child's GPU wrote are there in the parent's texture",
                WIDTH * HEIGHT
            );
            Ok(())
        } else {
            Err(format!("{wrong} of {} pixels differ", WIDTH * HEIGHT))
        }
    }

    pub fn child(name: &str) -> Result<(), String> {
        let surface = new_surface()?;
        let (device, queue) = wgpu_device()?;
        let texture = surface_texture(&device, &surface, wgpu::TextureUsages::COPY_DST)?;
        let mut data = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                data.extend_from_slice(&pattern(x, y));
            }
        }
        queue.write_texture(
            texture.as_image_copy(),
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 4),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([]);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        println!(
            "child {}: pattern written into the surface on the GPU",
            std::process::id()
        );

        let name = CString::new(name).unwrap();
        let mut parent: mach_port_t = MACH_PORT_NULL;
        // SAFETY: a C string and an out-pointer.
        let kr = unsafe { bootstrap_look_up(bootstrap_port, name.as_ptr(), &mut parent) };
        if kr != KERN_SUCCESS {
            return Err(format!("bootstrap_look_up failed ({kr})"));
        }
        let port = surface.create_mach_port();
        let mut message = PortMessage {
            header: mach_msg_header_t {
                msgh_bits: MACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, 0) | MACH_MSGH_BITS_COMPLEX,
                msgh_size: size_of::<PortMessage>() as u32,
                msgh_remote_port: parent,
                msgh_local_port: MACH_PORT_NULL,
                msgh_voucher_port: MACH_PORT_NULL,
                msgh_id: MESSAGE_ID,
            },
            body: mach_msg_body_t {
                msgh_descriptor_count: 1,
            },
            port: mach_msg_port_descriptor_t::new(port, MACH_MSG_TYPE_MOVE_SEND),
        };
        // SAFETY: a complete message to a send right.
        let kr = unsafe {
            mach_msg(
                &mut message.header,
                MACH_SEND_MSG,
                size_of::<PortMessage>() as u32,
                0,
                MACH_PORT_NULL,
                0,
                MACH_PORT_NULL,
            )
        };
        if kr != KERN_SUCCESS {
            return Err(format!("mach_msg send failed ({kr:#x})"));
        }
        println!("child: surface's port sent; staying until the parent is done");
        // The surface lives until the parent closes our stdin.
        let _ = std::io::stdin().read(&mut [0; 1]);
        drop(surface);
        Ok(())
    }

    fn new_surface() -> Result<CFRetained<IOSurfaceRef>, String> {
        let number = |n: i32| CFNumber::new_i32(n);
        let (width, height, bpe, format) = (
            number(WIDTH as i32),
            number(HEIGHT as i32),
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
        let values: [&CFType; 4] = [&width, &height, &bpe, &format];
        let properties = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
        // SAFETY: a dictionary of IOSurface's property keys and numbers.
        unsafe { IOSurfaceRef::new(properties.as_opaque()) }.ok_or_else(|| "IOSurfaceCreate failed".into())
    }

    fn wgpu_device() -> Result<(wgpu::Device, wgpu::Queue), String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster_block_on(instance.request_adapter(&Default::default())).map_err(|e| e.to_string())?;
        pollster_block_on(adapter.request_device(&Default::default())).map_err(|e| e.to_string())
    }

    fn pollster_block_on<F: std::future::Future>(future: F) -> F::Output {
        futures::executor::block_on(future)
    }

    /// The IOSurface as a wgpu texture of `device`: a Metal texture on the surface, wrapped.
    fn surface_texture(
        device: &wgpu::Device,
        surface: &IOSurfaceRef,
        usage: wgpu::TextureUsages,
    ) -> Result<wgpu::Texture, String> {
        // SAFETY: the device is Metal's (the instance has no other backend).
        let hal_device = unsafe { device.as_hal::<Metal>() }.ok_or("not a Metal device")?;
        let raw_device = hal_device.raw_device();
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::BGRA8Unorm,
                WIDTH as usize,
                HEIGHT as usize,
                false,
            )
        };
        descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite | MTLTextureUsage::RenderTarget);
        descriptor.setStorageMode(MTLStorageMode::Shared);
        let raw: Retained<ProtocolObject<dyn MTLTexture>> = raw_device
            .newTextureWithDescriptor_iosurface_plane(&descriptor, surface, 0)
            .ok_or("newTextureWithDescriptor:iosurface:plane: failed")?;
        // SAFETY: a 2D texture of the size and format described, alive as long as the hal texture.
        let hal_texture = unsafe {
            wgpu::hal::metal::Device::texture_from_raw(
                raw,
                wgpu::TextureFormat::Bgra8Unorm,
                objc2_metal::MTLTextureType::Type2D,
                1,
                1,
                wgpu::hal::CopyExtent {
                    width: WIDTH,
                    height: HEIGHT,
                    depth: 1,
                },
                None,
            )
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("IOSurface"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage,
            view_formats: &[],
        };
        // SAFETY: the hal texture is initialized (the IOSurface's contents are kept).
        Ok(unsafe { device.create_texture_from_hal::<Metal>(hal_texture, &desc, wgpu::TextureUses::COPY_SRC) })
    }

    /// Copies `texture` into a buffer on the GPU and reads it: BGRA rows of `WIDTH * 4`.
    fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Result<Vec<u8>, String> {
        let padded = (WIDTH * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(padded * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        let data = buffer.slice(..).get_mapped_range().map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for y in 0..HEIGHT as usize {
            let start = y * padded as usize;
            pixels.extend_from_slice(&data[start..start + WIDTH as usize * 4]);
        }
        Ok(pixels)
    }

    #[allow(dead_code)]
    fn unused(_: IOSurfaceLockOptions) {}
}
