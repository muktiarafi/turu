//! Turu: what draws the game's own texture without a window, shared by Android's screen
//! (`android/src/screen.rs`) and the client mode (`crate::client_screen`): the texture the player
//! renders into ([`MovieTexture`]) and the pass that draws it under Turu's UI ([`Blit`]).

pub use crate::blit::Blit;
use ruffle_render_wgpu::target::{RenderTarget, RenderTargetFrame};
use ruffle_render_wgpu::wgpu;
use std::sync::Arc;

/// The texture the game draws into (the desktop's `MovieView`).
#[derive(Debug)]
pub struct MovieTexture {
    blit: Arc<Blit>,
    texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

impl MovieTexture {
    pub fn new(blit: Arc<Blit>, device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Turu game"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind_group = blit.bind_group(device, &view);
        Self {
            blit,
            texture,
            bind_group,
        }
    }
}

impl RenderTarget for MovieTexture {
    type Frame = MovieFrame;

    fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = MovieTexture::new(self.blit.clone(), device, width, height);
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.texture.format()
    }

    fn width(&self) -> u32 {
        self.texture.width()
    }

    fn height(&self) -> u32 {
        self.texture.height()
    }

    fn get_next_texture(&mut self) -> Option<Self::Frame> {
        Some(MovieFrame(self.texture.create_view(&Default::default())))
    }

    fn submit<I: IntoIterator<Item = wgpu::CommandBuffer>>(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        command_buffers: I,
        _frame: Self::Frame,
    ) -> wgpu::SubmissionIndex {
        queue.submit(command_buffers)
    }

    /// Turu (Mugetsu 2.0.0's `MovieView`): with a present thread (a tab's client), the picture is
    /// finished and submitted there, and the screen's own picture of it is sent after it, through
    /// the same thread (`ClientScreen::set_present_thread`). Without one this isn't asked.
    fn submit_later(&self, _frame: Self::Frame) -> Result<ruffle_render_wgpu::target::LaterSubmit, Self::Frame> {
        Ok(Box::new(|queue, command_buffers| queue.submit(command_buffers)))
    }
}

#[derive(Debug)]
pub struct MovieFrame(wgpu::TextureView);

impl RenderTargetFrame for MovieFrame {
    fn into_view(self) -> wgpu::TextureView {
        self.0
    }

    fn view(&self) -> &wgpu::TextureView {
        &self.0
    }
}
