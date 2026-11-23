//! Turu: a client's pictures as GPU surfaces the shell draws from, on Windows and Linux
//! (docs/TAB_PROCESSES_PROPOSAL.md, M5; macOS has IOSurfaces, `shared_surface.rs`). By the
//! backend both processes run, which they must share, with the GPU (`ToShell::Gpu`):
//!
//! * **Vulkan, Linux**: the client makes each surface's image with exportable memory and sends the
//!   memory as an fd (`VK_KHR_external_memory_fd`, opaque) over the shell's Unix socket
//!   ([`listen`], `TURU_SHELL_SURFACES` names it), with its size and memory type; the shell makes
//!   the same image and imports the memory into it.
//! * **Vulkan, Windows**: the same, the memory as a Win32 handle (`VK_KHR_external_memory_win32`,
//!   opaque), whose value goes in `ToShell::SharedHandle`; the shell copies the handle out of the
//!   client it started (`DuplicateHandle`).
//! * **DX12, Windows**: a committed resource on a shared heap, simultaneous-access (each process
//!   tracks its states blind to the other's), whose shared handle goes the same way and is opened
//!   with `OpenSharedHandle`.
//!
//! Anything that fails sends the client back to pixels (`ToClient::NoSurfaces`): the shell then
//! copies them, as before. `TURU_NO_SHARED_SURFACES` set for the shell does that from the start.
//! Which surface holds the newest picture, and when the shell is done with one, go over the
//! socket as on macOS (`ToShell::Surface`, `ToClient::Release`). Written without the hardware to
//! run it: the first runs on each are the check (docs, M5).

#[cfg(windows)]
use std::collections::VecDeque;
use turu_egui::client_screen::SharedSurfaces;
use wgpu::hal::api::Vulkan;
use wgpu::hal::vulkan::TextureMemory;
use wgpu::hal::vulkan::{self as hal_vulkan};

/// Vulkan's handle type for the memory, by platform.
#[cfg(unix)]
const HANDLE_TYPE: ash::vk::ExternalMemoryHandleTypeFlags = ash::vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;
#[cfg(windows)]
const HANDLE_TYPE: ash::vk::ExternalMemoryHandleTypeFlags = ash::vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32;

/// How the pictures are shared: by the backend the client's GPU runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Vulkan,
    #[cfg(windows)]
    Dx12,
}

impl Route {
    /// The route for this backend, if there's one.
    pub fn for_backend(backend: wgpu::Backend) -> Option<Self> {
        match backend {
            wgpu::Backend::Vulkan => Some(Route::Vulkan),
            #[cfg(windows)]
            wgpu::Backend::Dx12 => Some(Route::Dx12),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            #[cfg(unix)]
            Route::Vulkan => "vulkan-fd",
            #[cfg(windows)]
            Route::Vulkan => "vulkan-win32",
            #[cfg(windows)]
            Route::Dx12 => "dx12",
        }
    }

    #[cfg(windows)]
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "vulkan-win32" => Some(Route::Vulkan),
            "dx12" => Some(Route::Dx12),
            _ => None,
        }
    }
}

/// The surfaces' image, the same on both sides (an imported image must be made as the exported
/// one was): BGRA, drawn into by the client, sampled by the shell.
fn image_info<'a>(
    width: u32,
    height: u32,
    external: &'a mut ash::vk::ExternalMemoryImageCreateInfo<'a>,
) -> ash::vk::ImageCreateInfo<'a> {
    use ash::vk;
    vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::B8G8R8A8_UNORM)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST,
        )
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(external)
}

/// The wgpu descriptor of a surface's texture, for `usage` on this side.
fn texture_desc(width: u32, height: u32, usage: wgpu::TextureUsages) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: Some("Turu shared picture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage,
        view_formats: &[],
    }
}

/// The hal side of [`texture_desc`].
fn hal_desc(width: u32, height: u32, uses: wgpu::TextureUses) -> wgpu::hal::TextureDescriptor<'static> {
    wgpu::hal::TextureDescriptor {
        label: Some("Turu shared picture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: uses,
        memory_flags: wgpu::hal::MemoryFlags::empty(),
        view_formats: Vec::new(),
    }
}

/// A device-local memory type among `bits`.
fn memory_type(device: &hal_vulkan::Device, bits: u32) -> Option<u32> {
    let instance = device.shared_instance().raw_instance();
    // SAFETY: the device's own physical device.
    let properties = unsafe { instance.get_physical_device_memory_properties(device.raw_physical_device()) };
    (0..properties.memory_type_count).find(|&index| {
        bits & (1 << index) != 0
            && properties.memory_types[index as usize]
                .property_flags
                .contains(ash::vk::MemoryPropertyFlags::DEVICE_LOCAL)
    })
}

/// Whether the device can export and import the memory: its extension is on, and the image's
/// format takes exportable and importable memory.
fn vulkan_can_share(device: &hal_vulkan::Device) -> bool {
    use ash::vk;
    #[cfg(unix)]
    let extension = ash::khr::external_memory_fd::NAME;
    #[cfg(windows)]
    let extension = ash::khr::external_memory_win32::NAME;
    if !device.enabled_device_extensions().contains(&extension) {
        tracing::info!("Shared surfaces: the Vulkan device has no {extension:?}");
        return false;
    }
    let instance = device.shared_instance().raw_instance();
    let mut external_info = vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(HANDLE_TYPE);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(vk::Format::B8G8R8A8_UNORM)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(
            vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST,
        )
        .push_next(&mut external_info);
    let mut external = vk::ExternalImageFormatProperties::default();
    let mut properties = vk::ImageFormatProperties2::default().push_next(&mut external);
    // SAFETY: the device's own physical device; plain structs.
    let result = unsafe {
        instance.get_physical_device_image_format_properties2(device.raw_physical_device(), &info, &mut properties)
    };
    let features = external.external_memory_properties.external_memory_features;
    let both = vk::ExternalMemoryFeatureFlags::EXPORTABLE | vk::ExternalMemoryFeatureFlags::IMPORTABLE;
    if result.is_err() || !features.contains(both) {
        tracing::info!("Shared surfaces: the GPU can't share this image's memory ({result:?}, {features:?})");
        return false;
    }
    true
}

/// What a client sends for a Vulkan surface: the memory's handle (an fd or a Win32 handle,
/// owned by the client until sent), its size and type.
struct VulkanExport {
    texture: wgpu::Texture,
    handle: u64,
    size: u64,
    memory_type: u32,
}

/// Makes a Vulkan surface of `width` by `height` whose memory can go to another process.
fn vulkan_export(device: &wgpu::Device, width: u32, height: u32) -> Option<VulkanExport> {
    use ash::vk;
    // SAFETY: only Vulkan's device is used, if it is one.
    let hal = unsafe { device.as_hal::<Vulkan>() }?;
    let raw = hal.raw_device();
    let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);
    let info = image_info(width, height, &mut external);
    // SAFETY: a valid create info; the image is destroyed below on failure, else by wgpu.
    let image = unsafe { raw.create_image(&info, None) }.ok()?;
    let fail = |memory: Option<vk::DeviceMemory>| {
        // SAFETY: made here and not used elsewhere.
        unsafe {
            if let Some(memory) = memory {
                raw.free_memory(memory, None);
            }
            raw.destroy_image(image, None);
        }
        None
    };
    // SAFETY: the image just made.
    let requirements = unsafe { raw.get_image_memory_requirements(image) };
    let Some(memory_type) = memory_type(&hal, requirements.memory_type_bits) else {
        return fail(None);
    };
    let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(HANDLE_TYPE);
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type)
        .push_next(&mut export)
        .push_next(&mut dedicated);
    // SAFETY: a valid allocate info for the image's memory.
    let Ok(memory) = (unsafe { raw.allocate_memory(&allocate, None) }) else {
        return fail(None);
    };
    // SAFETY: memory allocated for this image.
    if unsafe { raw.bind_image_memory(image, memory, 0) }.is_err() {
        return fail(Some(memory));
    }
    let instance = hal.shared_instance().raw_instance();
    #[cfg(unix)]
    let handle = {
        let functions = ash::khr::external_memory_fd::Device::new(instance, raw);
        let get = vk::MemoryGetFdInfoKHR::default()
            .memory(memory)
            .handle_type(HANDLE_TYPE);
        // SAFETY: exportable memory of this device; the fd is the caller's.
        unsafe { functions.get_memory_fd(&get) }.map(|fd| fd as u64)
    };
    #[cfg(windows)]
    let handle = {
        let functions = ash::khr::external_memory_win32::Device::new(instance, raw);
        let get = vk::MemoryGetWin32HandleInfoKHR::default()
            .memory(memory)
            .handle_type(HANDLE_TYPE);
        // SAFETY: exportable memory of this device; the handle is the caller's.
        unsafe { functions.get_memory_win32_handle(&get) }.map(|handle| handle as u64)
    };
    let Ok(handle) = handle else {
        tracing::warn!("Shared surfaces: Vulkan didn't export the memory");
        return fail(Some(memory));
    };
    // SAFETY: the image and its memory, owned by the hal texture from here on.
    let hal_texture = unsafe {
        hal.texture_from_raw(
            image,
            &hal_desc(width, height, wgpu::TextureUses::COLOR_TARGET),
            None,
            TextureMemory::Dedicated(memory),
        )
    };
    drop(hal);
    // SAFETY: the hal texture matches the descriptor.
    let texture = unsafe {
        device.create_texture_from_hal::<Vulkan>(
            hal_texture,
            &texture_desc(width, height, wgpu::TextureUsages::RENDER_ATTACHMENT),
            wgpu::TextureUses::UNINITIALIZED,
        )
    };
    Some(VulkanExport {
        texture,
        handle,
        size: requirements.size,
        memory_type,
    })
}

/// Makes the image of a client's Vulkan surface on the shell's device, with the client's memory
/// (`handle`, `size` bytes of type `memory_type`). An fd is Vulkan's once imported, closed here
/// otherwise; a Win32 handle stays the caller's either way.
fn vulkan_import(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    handle: u64,
    size: u64,
    memory_type_index: u32,
) -> Option<wgpu::Texture> {
    use ash::vk;
    #[cfg(unix)]
    let close_fd = || {
        // SAFETY: the fd this function was given and didn't hand to Vulkan.
        unsafe { libc::close(handle as i32) };
    };
    #[cfg(windows)]
    let close_fd = || {};
    // SAFETY: only Vulkan's device is used, if it is one.
    let Some(hal) = (unsafe { device.as_hal::<Vulkan>() }) else {
        close_fd();
        return None;
    };
    if !vulkan_can_share(&hal) {
        close_fd();
        return None;
    }
    let raw = hal.raw_device();
    let mut external = vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);
    let info = image_info(width, height, &mut external);
    // SAFETY: a valid create info.
    let Ok(image) = (unsafe { raw.create_image(&info, None) }) else {
        close_fd();
        return None;
    };
    // SAFETY: the image just made.
    let requirements = unsafe { raw.get_image_memory_requirements(image) };
    if requirements.memory_type_bits & (1 << memory_type_index) == 0 || requirements.size > size {
        tracing::warn!(
            "Shared surfaces: the client's memory (type {memory_type_index}, {size} bytes) doesn't fit the image \
             here (types {:b}, {} bytes)",
            requirements.memory_type_bits,
            requirements.size
        );
        // SAFETY: made here, not used.
        unsafe { raw.destroy_image(image, None) };
        close_fd();
        return None;
    }
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    #[cfg(unix)]
    let mut import = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(HANDLE_TYPE)
        .fd(handle as i32);
    #[cfg(windows)]
    let mut import = vk::ImportMemoryWin32HandleInfoKHR::default()
        .handle_type(HANDLE_TYPE)
        .handle(handle as isize);
    // The exporter's size and type, as the spec asks of opaque handles.
    let allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(size)
        .memory_type_index(memory_type_index)
        .push_next(&mut import)
        .push_next(&mut dedicated);
    // SAFETY: a valid allocate info importing the client's memory.
    let memory = match unsafe { raw.allocate_memory(&allocate, None) } {
        Ok(memory) => memory,
        Err(e) => {
            tracing::warn!("Shared surfaces: Vulkan didn't import the client's memory ({e})");
            // SAFETY: made here, not used.
            unsafe { raw.destroy_image(image, None) };
            close_fd();
            return None;
        }
    };
    // SAFETY: memory imported for this image.
    if let Err(e) = unsafe { raw.bind_image_memory(image, memory, 0) } {
        tracing::warn!("Shared surfaces: couldn't bind the client's memory ({e})");
        // SAFETY: made here, not used.
        unsafe {
            raw.free_memory(memory, None);
            raw.destroy_image(image, None);
        }
        return None;
    }
    // SAFETY: the image and its memory, owned by the hal texture from here on.
    let hal_texture = unsafe {
        hal.texture_from_raw(
            image,
            &hal_desc(width, height, wgpu::TextureUses::RESOURCE),
            None,
            TextureMemory::Dedicated(memory),
        )
    };
    drop(hal);
    // SAFETY: the hal texture matches the descriptor; its contents are the client's.
    Some(unsafe {
        device.create_texture_from_hal::<Vulkan>(
            hal_texture,
            &texture_desc(width, height, wgpu::TextureUsages::TEXTURE_BINDING),
            wgpu::TextureUses::RESOURCE,
        )
    })
}

/// A client's surface as the shell got it, opened on its GPU when first drawn.
pub struct Surface {
    route: Route,
    width: u32,
    height: u32,
    /// An fd (Linux) or a handle in the shell's process (Windows); taken when opened.
    handle: Option<u64>,
    size: u64,
    memory_type: u32,
}

impl Surface {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The surface as a texture of `device`; `None` if it couldn't be opened.
    pub fn texture(&mut self, device: &wgpu::Device) -> Option<wgpu::Texture> {
        let handle = self.handle.take()?;
        let texture = match self.route {
            Route::Vulkan => vulkan_import(device, self.width, self.height, handle, self.size, self.memory_type),
            #[cfg(windows)]
            Route::Dx12 => windows::dx12_open(device, self.width, self.height, handle),
        };
        #[cfg(windows)]
        windows::close(handle);
        texture
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            #[cfg(unix)]
            // SAFETY: an fd this surface owns, never handed to Vulkan.
            unsafe {
                libc::close(handle as i32);
            }
            #[cfg(windows)]
            windows::close(handle);
        }
    }
}

/// A client's surfaces, made on its GPU and handed to the shell.
pub struct Sender {
    route: Route,
    next_id: u64,
    #[cfg(unix)]
    socket: std::os::unix::net::UnixStream,
    #[cfg(windows)]
    out: turu_frontend::shell::FrameSender,
    /// Handles sent lately, closed once newer ones have gone (the shell copies each as it comes).
    #[cfg(windows)]
    sent: VecDeque<u64>,
}

impl Sender {
    /// Shares surfaces with the shell if the client's GPU runs a backend there's a route for.
    /// `surfaces` is the shell's `TURU_SHELL_SURFACES`.
    #[cfg(unix)]
    pub fn connect(
        device: &wgpu::Device,
        backend: wgpu::Backend,
        surfaces: &str,
        _out: &turu_frontend::shell::FrameSender,
    ) -> Option<Self> {
        let route = usable_route(device, backend)?;
        let socket = std::os::unix::net::UnixStream::connect(surfaces)
            .inspect_err(|e| tracing::warn!("Couldn't reach the shell's surfaces at {surfaces} ({e}): sending pixels"))
            .ok()?;
        Some(Self {
            route,
            next_id: 1,
            socket,
        })
    }

    #[cfg(windows)]
    pub fn connect(
        device: &wgpu::Device,
        backend: wgpu::Backend,
        _surfaces: &str,
        out: &turu_frontend::shell::FrameSender,
    ) -> Option<Self> {
        let route = usable_route(device, backend)?;
        Some(Self {
            route,
            next_id: 1,
            out: out.clone(),
            sent: VecDeque::new(),
        })
    }

    pub fn route(&self) -> Route {
        self.route
    }
}

/// The route for the client's backend, if its GPU can share.
fn usable_route(device: &wgpu::Device, backend: wgpu::Backend) -> Option<Route> {
    let route = Route::for_backend(backend);
    match route {
        Some(Route::Vulkan) => {
            // SAFETY: only Vulkan's device is used, if it is one.
            let hal = unsafe { device.as_hal::<Vulkan>() }?;
            vulkan_can_share(&hal).then_some(Route::Vulkan)
        }
        #[cfg(windows)]
        Some(Route::Dx12) => Some(Route::Dx12),
        None => {
            tracing::info!("Shared surfaces: none for {backend:?}: sending pixels");
            None
        }
    }
}

impl SharedSurfaces for Sender {
    fn make(&mut self, device: &wgpu::Device, (width, height): (u32, u32)) -> Option<(u64, wgpu::Texture)> {
        let id = self.next_id;
        self.next_id += 1;
        match self.route {
            Route::Vulkan => {
                let export = vulkan_export(device, width, height)?;
                #[cfg(unix)]
                {
                    let sent = unix::send(
                        &self.socket,
                        export.handle as i32,
                        &unix::Header {
                            pid: std::process::id(),
                            width,
                            height,
                            memory_type: export.memory_type,
                            id,
                            size: export.size,
                        },
                    );
                    // The shell got its own copy (or none): this one is closed either way.
                    // SAFETY: the fd Vulkan exported for us.
                    unsafe { libc::close(export.handle as i32) };
                    if let Err(e) = sent {
                        tracing::warn!("Couldn't hand a surface to the shell ({e})");
                        return None;
                    }
                }
                #[cfg(windows)]
                self.send_handle(id, width, height, export.handle, export.size, export.memory_type);
                Some((id, export.texture))
            }
            #[cfg(windows)]
            Route::Dx12 => {
                let (texture, handle) = windows::dx12_export(device, width, height)?;
                self.send_handle(id, width, height, handle, 0, 0);
                Some((id, texture))
            }
        }
    }
}

#[cfg(windows)]
impl Sender {
    fn send_handle(&mut self, id: u64, width: u32, height: u32, handle: u64, size: u64, memory_type: u32) {
        self.out.send(turu_frontend::shell::ToShell::SharedHandle {
            id,
            route: self.route.name().to_owned(),
            handle,
            width,
            height,
            size,
            memory_type,
        });
        self.sent.push_back(handle);
        while self.sent.len() > 2 * turu_frontend::shell::SHARED_POOL + 2 {
            if let Some(old) = self.sent.pop_front() {
                windows::close(old);
            }
        }
    }
}

#[cfg(windows)]
impl Drop for Sender {
    fn drop(&mut self) {
        for handle in self.sent.drain(..) {
            windows::close(handle);
        }
    }
}

/// The shell's side on Windows: a client's `SharedHandle`, copied out of its process.
#[cfg(windows)]
pub fn take_handle(
    child: &std::process::Child,
    route: &str,
    handle: u64,
    width: u32,
    height: u32,
    size: u64,
    memory_type: u32,
) -> Option<Surface> {
    let Some(route) = Route::from_name(route) else {
        tracing::warn!("Shell: a surface by an unknown route ({route})");
        return None;
    };
    let handle = windows::duplicate_from(child, handle)?;
    Some(Surface {
        route,
        width,
        height,
        handle: Some(handle),
        size,
        memory_type,
    })
}

#[cfg(windows)]
mod windows {
    use std::os::windows::io::AsRawHandle;
    use wgpu::hal::api::Dx12;
    use windows::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, GENERIC_ALL, HANDLE};
    use windows::Win32::Graphics::Direct3D12::{
        D3D12_HEAP_FLAG_SHARED, D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DESC,
        D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS, D3D12_RESOURCE_STATE_COMMON, D3D12_TEXTURE_LAYOUT_UNKNOWN,
        ID3D12Resource,
    };
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::core::PCWSTR;

    pub fn close(handle: u64) {
        // SAFETY: a handle this process owns.
        let _ = unsafe { CloseHandle(HANDLE(handle as _)) };
    }

    /// Copies `handle` out of `child`'s process into this one.
    pub fn duplicate_from(child: &std::process::Child, handle: u64) -> Option<u64> {
        let mut copy = HANDLE::default();
        // SAFETY: the child's process handle (all access, as it was started here) and a handle
        // value in it.
        let result = unsafe {
            DuplicateHandle(
                HANDLE(child.as_raw_handle() as _),
                HANDLE(handle as _),
                GetCurrentProcess(),
                &mut copy,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        };
        match result {
            Ok(()) => Some(copy.0 as u64),
            Err(e) => {
                tracing::warn!("Shell: couldn't copy a client's surface handle ({e})");
                None
            }
        }
    }

    /// A shared, simultaneous-access BGRA texture on the client's D3D12 device, and its shared
    /// handle (the caller's).
    pub fn dx12_export(device: &wgpu::Device, width: u32, height: u32) -> Option<(wgpu::Texture, u64)> {
        // SAFETY: only D3D12's device is used, if it is one.
        let hal = unsafe { device.as_hal::<Dx12>() }?;
        let raw = hal.raw_device();
        let heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: u64::from(width),
            Height: height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET | D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS,
        };
        let mut resource: Option<ID3D12Resource> = None;
        // SAFETY: valid descriptions; the resource comes back in `resource`.
        let made = unsafe {
            raw.CreateCommittedResource(
                &heap,
                D3D12_HEAP_FLAG_SHARED,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
        };
        let resource = match (made, resource) {
            (Ok(()), Some(resource)) => resource,
            (result, _) => {
                tracing::warn!("Shared surfaces: D3D12 didn't make a shared texture ({result:?})");
                return None;
            }
        };
        // SAFETY: a resource on a shared heap of this device.
        let handle = match unsafe { raw.CreateSharedHandle(&resource, None, GENERIC_ALL.0, PCWSTR::null()) } {
            Ok(handle) => handle,
            Err(e) => {
                tracing::warn!("Shared surfaces: no shared handle for the texture ({e})");
                return None;
            }
        };
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        // SAFETY: a 2D BGRA texture of that size, one mip, one sample.
        let hal_texture = unsafe {
            wgpu::hal::dx12::Device::texture_from_raw(
                resource,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureDimension::D2,
                extent,
                1,
                1,
            )
        };
        drop(hal);
        // SAFETY: the hal texture matches the descriptor.
        let texture = unsafe {
            device.create_texture_from_hal::<Dx12>(
                hal_texture,
                &super::texture_desc(width, height, wgpu::TextureUsages::RENDER_ATTACHMENT),
                wgpu::TextureUses::UNINITIALIZED,
            )
        };
        Some((texture, handle.0 as u64))
    }

    /// The client's shared texture (`handle`, now the shell's) on the shell's D3D12 device.
    pub fn dx12_open(device: &wgpu::Device, width: u32, height: u32, handle: u64) -> Option<wgpu::Texture> {
        // SAFETY: only D3D12's device is used, if it is one.
        let hal = unsafe { device.as_hal::<Dx12>() }?;
        let mut resource: Option<ID3D12Resource> = None;
        // SAFETY: a shared handle this process owns.
        let opened = unsafe { hal.raw_device().OpenSharedHandle(HANDLE(handle as _), &mut resource) };
        let resource = match (opened, resource) {
            (Ok(()), Some(resource)) => resource,
            (result, _) => {
                tracing::warn!("Shell: D3D12 couldn't open a client's texture ({result:?})");
                return None;
            }
        };
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        // SAFETY: the client's 2D BGRA texture of that size.
        let hal_texture = unsafe {
            wgpu::hal::dx12::Device::texture_from_raw(
                resource,
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureDimension::D2,
                extent,
                1,
                1,
            )
        };
        drop(hal);
        // SAFETY: the hal texture matches the descriptor; its contents are the client's.
        Some(unsafe {
            device.create_texture_from_hal::<Dx12>(
                hal_texture,
                &super::texture_desc(width, height, wgpu::TextureUsages::TEXTURE_BINDING),
                wgpu::TextureUses::RESOURCE,
            )
        })
    }
}

/// The shell's side on Linux: a Unix socket the clients send their surfaces' fds over.
#[cfg(unix)]
pub fn listen(got: impl Fn(u32, u64, Surface) + Send + 'static) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let path = crate::shell_sockets::path(&dir, std::process::id());
    let _ = std::fs::remove_file(&path);
    // Turu: a shell that crashed (or was killed) left its socket behind: removed here, by the next.
    crate::shell_sockets::remove_stale(&dir, std::process::id());
    let listener = std::os::unix::net::UnixListener::bind(&path)
        .inspect_err(|e| {
            tracing::warn!(
                "Shell: couldn't listen at {}: {e}: clients will send pixels",
                path.display()
            )
        })
        .ok()?;
    // Only this user's processes may hand it surfaces.
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    // One callback for every client's thread (the event loop's proxy needn't be `Sync`).
    let got = std::sync::Arc::new(std::sync::Mutex::new(got));
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Surfaces".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let got = got.clone();
                let _ = std::thread::Builder::new()
                    .name("Turu Shell Surface Client".into())
                    .spawn(move || {
                        while let Some((fd, header)) = unix::receive(&stream) {
                            let got = got.lock().unwrap_or_else(|e| e.into_inner());
                            got(
                                header.pid,
                                header.id,
                                Surface {
                                    route: Route::Vulkan,
                                    width: header.width,
                                    height: header.height,
                                    handle: Some(fd as u64),
                                    size: header.size,
                                    memory_type: header.memory_type,
                                },
                            );
                        }
                    });
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Shell: couldn't start receiving surfaces: {e}");
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

#[cfg(unix)]
mod unix {
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    /// What goes with a surface's fd.
    pub struct Header {
        pub pid: u32,
        pub width: u32,
        pub height: u32,
        pub memory_type: u32,
        pub id: u64,
        pub size: u64,
    }

    const LEN: usize = 32;

    impl Header {
        fn to_bytes(&self) -> [u8; LEN] {
            let mut bytes = [0; LEN];
            bytes[0..4].copy_from_slice(&self.pid.to_le_bytes());
            bytes[4..8].copy_from_slice(&self.width.to_le_bytes());
            bytes[8..12].copy_from_slice(&self.height.to_le_bytes());
            bytes[12..16].copy_from_slice(&self.memory_type.to_le_bytes());
            bytes[16..24].copy_from_slice(&self.id.to_le_bytes());
            bytes[24..32].copy_from_slice(&self.size.to_le_bytes());
            bytes
        }

        fn from_bytes(bytes: &[u8; LEN]) -> Self {
            let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
            let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
            Self {
                pid: u32_at(0),
                width: u32_at(4),
                height: u32_at(8),
                memory_type: u32_at(12),
                id: u64_at(16),
                size: u64_at(24),
            }
        }
    }

    /// Room for one fd's control message.
    fn control_len() -> usize {
        // SAFETY: a pure size computation.
        unsafe { libc::CMSG_SPACE(size_of::<libc::c_int>() as u32) as usize }
    }

    /// Sends `fd` (duplicated into the receiver) with `header`. The stream carries one header after
    /// another, so a header the kernel took only part of is finished: the next would be read out of
    /// step otherwise.
    pub fn send(stream: &UnixStream, fd: i32, header: &Header) -> std::io::Result<()> {
        let mut bytes = header.to_bytes();
        let mut iov = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: LEN,
        };
        let mut control = vec![0u8; control_len()];
        // SAFETY: a zeroed msghdr, filled with buffers that outlive the call.
        let sent = unsafe {
            let mut message: libc::msghdr = std::mem::zeroed();
            message.msg_iov = &mut iov;
            message.msg_iovlen = 1;
            message.msg_control = control.as_mut_ptr().cast();
            message.msg_controllen = control.len() as _;
            let cmsg = libc::CMSG_FIRSTHDR(&message);
            (*cmsg).cmsg_level = libc::SOL_SOCKET;
            (*cmsg).cmsg_type = libc::SCM_RIGHTS;
            (*cmsg).cmsg_len = libc::CMSG_LEN(size_of::<libc::c_int>() as u32) as _;
            std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::c_int>(), fd);
            libc::sendmsg(stream.as_raw_fd(), &message, 0)
        };
        let Ok(sent) = usize::try_from(sent) else {
            return Err(std::io::Error::last_os_error());
        };
        // The fd went with the first byte: the rest of the header follows without it.
        let mut stream = stream;
        stream.write_all(&bytes[sent.min(LEN)..])
    }

    /// The next fd and header; `None` when the client is gone.
    pub fn receive(stream: &UnixStream) -> Option<(i32, Header)> {
        loop {
            let mut bytes = [0u8; LEN];
            let mut iov = libc::iovec {
                iov_base: bytes.as_mut_ptr().cast(),
                iov_len: LEN,
            };
            let mut control = vec![0u8; control_len()];
            // SAFETY: a zeroed msghdr with buffers that outlive the call; the fd, if any, is
            // taken from the control message as the kernel wrote it.
            let (read, fd) = unsafe {
                let mut message: libc::msghdr = std::mem::zeroed();
                message.msg_iov = &mut iov;
                message.msg_iovlen = 1;
                message.msg_control = control.as_mut_ptr().cast();
                message.msg_controllen = control.len() as _;
                let read = libc::recvmsg(stream.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC);
                let cmsg = libc::CMSG_FIRSTHDR(&message);
                let fd = (!cmsg.is_null()
                    && (*cmsg).cmsg_level == libc::SOL_SOCKET
                    && (*cmsg).cmsg_type == libc::SCM_RIGHTS)
                    .then(|| std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<libc::c_int>()));
                (read, fd)
            };
            let read = usize::try_from(read).ok().filter(|&read| read > 0)?;
            if let Some(fd) = fd {
                // A header split by the kernel: its fd came with the first part, the rest follows.
                let mut stream = stream;
                if read < LEN && stream.read_exact(&mut bytes[read..]).is_err() {
                    // SAFETY: an fd the kernel just gave this process.
                    unsafe { libc::close(fd) };
                    return None;
                }
                return Some((fd, Header::from_bytes(&bytes)));
            }
            tracing::warn!("Shell: a surface message without its fd; ignored");
        }
    }
}
