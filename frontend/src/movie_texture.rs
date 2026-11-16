//! Turu: what draws the game's own texture without a window, shared by Android's screen
//! (`android/src/screen.rs`) and the client mode (`crate::client_screen`): the texture the player
//! renders into ([`MovieTexture`]) and the pass that draws it under Turu's UI ([`Blit`]).

use ruffle_render_wgpu::target::{RenderTarget, RenderTargetFrame};
use ruffle_render_wgpu::wgpu;
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Draws a texture over the render pass's viewport: the desktop's `blit.wgsl`. The pipeline,
/// which depends on the surface's format, is the screen's ([`Blit::pipeline`]).
pub struct Blit {
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertices: wgpu::Buffer,
}

impl Blit {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Turu blit"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "../../app/src/gui/blit.wgsl"
            ))),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // The whole viewport: x y u v.
        let quad: [[f32; 4]; 6] = [
            [-1.0, 1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 0.0],
            [1.0, -1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0, 1.0],
            [-1.0, -1.0, 0.0, 1.0],
            [-1.0, 1.0, 0.0, 0.0],
        ];
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quad),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self {
            module,
            layout,
            sampler,
            vertices,
        }
    }

    pub fn pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        self.pipeline_blending(device, format, wgpu::BlendState::REPLACE)
    }

    /// [`Self::pipeline`] with another blend: the desktop shell draws a client's picture with
    /// premultiplied alpha over the Grid View tiles (`app/src/shell.rs`).
    pub fn pipeline_blending(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        blend: wgpu::BlendState,
    ) -> wgpu::RenderPipeline {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&self.layout)],
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Turu blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &self.module,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 4 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                })],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.module,
                entry_point: Some(if format.is_srgb() {
                    "fs_main_srgb_framebuffer"
                } else {
                    "fs_main_linear_framebuffer"
                }),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// What [`Self::draw`] takes to draw `view`.
    pub fn bind_group(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    pub fn draw(
        &self,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        pass: &mut wgpu::RenderPass<'static>,
    ) {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..6, 0..1);
    }
}

/// The texture the game draws into (the desktop's `MovieView`).
#[derive(Debug)]
pub struct MovieTexture {
    blit: Arc<Blit>,
    texture: wgpu::Texture,
    pub bind_group: wgpu::BindGroup,
}

impl std::fmt::Debug for Blit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blit")
    }
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
