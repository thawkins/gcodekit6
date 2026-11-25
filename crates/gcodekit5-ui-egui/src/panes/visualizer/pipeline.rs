//! The visualizer's wgpu drawing: pipelines, buffers and the egui paint callback.
//!
//! [`install`] builds the pipelines for the window's colour format once and keeps them in
//! the egui renderer's callback resources (as eframe's custom-3D pattern does). Each frame
//! the pane adds a [`VisualizerCallback`]; its `prepare` writes one small uniform (camera,
//! progress) and uploads the toolpath only when a new program was built, so a frame costs the
//! same for a million-line program as for ten lines. Its `paint` draws, in order: the grid
//! and origin axes, the toolpath, and the tool marker.
//!
//! The shaders are WGSL ports of the GTK `ui/gtk/shaders.rs` line shader. The callback draws
//! inside egui's render pass, which has no depth buffer, so later draws cover earlier ones
//! (the GTK view also draws the grid and the tool marker without depth testing).
//!
//! Dependencies: `egui-wgpu` (callback, wgpu), `bytemuck`, `glam`.

use std::sync::Arc;

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};

use super::toolpath::{ToolpathMesh, ToolpathVertex};

/// Grid size (mm, centred on the origin), as GTK draws it.
const GRID_SIZE: f32 = 4000.0;
/// Grid spacing in mm (GTK's default).
const GRID_STEP: f32 = 50.0;
/// Grid colour (GTK's 30% grey).
const GRID_COLOR: [u8; 4] = [77, 77, 77, 255];
/// Length of the origin axes in mm.
const AXIS_LENGTH: f32 = 100.0;
/// Tool marker colour: red at 40% opacity, as GTK draws it.
const MARKER_COLOR: [u8; 4] = [255, 0, 0, 102];
/// Tool marker cone height and radius in mm, and its number of sides.
const MARKER_HEIGHT: f32 = 10.0;
const MARKER_RADIUS: f32 = 1.5;
const MARKER_SIDES: usize = 16;

/// One grid, axis or marker vertex.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneVertex {
    /// Position in mm.
    pub pos: [f32; 3],
    /// sRGB colour with alpha.
    pub color: [u8; 4],
}

/// The shaders' uniform block.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    done_color: [f32; 4],
    progress: u32,
    srgb_target: u32,
    pad: [u32; 2],
}

/// The scene's GPU objects, kept in the egui renderer's callback resources.
struct GpuResources {
    toolpath_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    marker_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Grid and axes, built once.
    scene: wgpu::Buffer,
    scene_count: u32,
    /// The tool marker, rewritten when the tool moves.
    marker: wgpu::Buffer,
    /// The uploaded toolpath: its generation, buffer and vertex count.
    toolpath: Option<(u64, Option<wgpu::Buffer>, u32)>,
    srgb_target: bool,
}

/// The grid lines (Z = 0) and the X, Y and Z axes from the origin.
fn scene_vertices() -> Vec<SceneVertex> {
    let mut v = Vec::new();
    let mut line = |a: Vec3, b: Vec3, color: [u8; 4]| {
        v.push(SceneVertex {
            pos: a.to_array(),
            color,
        });
        v.push(SceneVertex {
            pos: b.to_array(),
            color,
        });
    };
    let half = GRID_SIZE / 2.0;
    let steps = (GRID_SIZE / GRID_STEP) as i32;
    for i in 0..=steps {
        let p = -half + i as f32 * GRID_STEP;
        line(
            Vec3::new(-half, p, 0.0),
            Vec3::new(half, p, 0.0),
            GRID_COLOR,
        );
        line(
            Vec3::new(p, -half, 0.0),
            Vec3::new(p, half, 0.0),
            GRID_COLOR,
        );
    }
    let axes = [
        (Vec3::X, [255, 0, 0, 255]),
        (Vec3::Y, [0, 255, 0, 255]),
        (Vec3::Z, [0, 0, 255, 255]),
    ];
    for (axis, color) in axes {
        line(Vec3::ZERO, axis * AXIS_LENGTH, color);
    }
    v
}

/// The tool marker: a cone with its tip at `tip` and its base 10 mm above.
fn marker_vertices(tip: Vec3) -> Vec<SceneVertex> {
    let base = tip + Vec3::Z * MARKER_HEIGHT;
    let rim = |i: usize| {
        let a = i as f32 / MARKER_SIDES as f32 * std::f32::consts::TAU;
        base + Vec3::new(MARKER_RADIUS * a.cos(), MARKER_RADIUS * a.sin(), 0.0)
    };
    let vertex = |p: Vec3| SceneVertex {
        pos: p.to_array(),
        color: MARKER_COLOR,
    };
    (0..MARKER_SIDES)
        .flat_map(|i| {
            let (p1, p2) = (rim(i), rim(i + 1));
            [tip, p1, p2, base, p2, p1].map(vertex)
        })
        .collect()
}

/// Number of tool marker vertices.
const MARKER_VERTICES: usize = MARKER_SIDES * 6;

/// A render pipeline drawing `topology` with `shader`'s `vs_main` and `fs_main`.
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    vertex: wgpu::VertexBufferLayout<'_>,
    topology: wgpu::PrimitiveTopology,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("gcodekit5_visualizer"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(vertex)],
        },
        primitive: wgpu::PrimitiveState {
            topology,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// Builds the visualizer's pipelines for `render_state`'s colour format and keeps them in its
/// renderer, so [`VisualizerCallback`] can draw. Without it the visualizer draws only its
/// background and overlays.
///
/// # Arguments
/// * `render_state` - The wgpu state of the window (eframe's, or a test renderer's).
pub fn install(render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;
    let format = render_state.target_format;
    let toolpath_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/toolpath.wgsl"));
    let grid_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/grid.wgsl"));
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gcodekit5_visualizer_uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gcodekit5_visualizer_bind_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gcodekit5_visualizer_bind_group"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniforms.as_entire_binding(),
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gcodekit5_visualizer_layout"),
        bind_group_layouts: &[Some(&bind_layout)],
        immediate_size: 0,
    });
    let toolpath_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ToolpathVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4, 2 => Uint32],
    };
    let scene_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SceneVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Unorm8x4],
    };
    let lines = wgpu::PrimitiveTopology::LineList;
    let scene = scene_vertices();
    let resources = GpuResources {
        toolpath_pipeline: pipeline(
            device,
            &layout,
            &toolpath_shader,
            toolpath_layout,
            lines,
            format,
        ),
        line_pipeline: pipeline(
            device,
            &layout,
            &grid_shader,
            scene_layout.clone(),
            lines,
            format,
        ),
        marker_pipeline: pipeline(
            device,
            &layout,
            &grid_shader,
            scene_layout,
            wgpu::PrimitiveTopology::TriangleList,
            format,
        ),
        uniforms,
        bind_group,
        scene: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gcodekit5_visualizer_scene"),
            contents: bytemuck::cast_slice(&scene),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        scene_count: scene.len() as u32,
        marker: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gcodekit5_visualizer_marker"),
            size: (MARKER_VERTICES * std::mem::size_of::<SceneVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        toolpath: None,
        srgb_target: format.is_srgb(),
    };
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(resources);
}

/// A toolpath built for one program, tagged so it is uploaded once.
#[derive(Debug, Clone)]
pub struct UploadedMesh {
    /// Changes whenever a new program's mesh is built.
    pub generation: u64,
    /// The vertices.
    pub mesh: Arc<ToolpathMesh>,
}

/// One frame's drawing of the visualizer.
#[derive(Debug, Clone)]
pub struct VisualizerCallback {
    /// Projection times view.
    pub view_proj: Mat4,
    /// Program lines acknowledged so far; those are drawn in `done_color`.
    pub progress: u32,
    /// sRGB colour of the acknowledged lines.
    pub done_color: [f32; 4],
    /// The toolpath, if a program is loaded.
    pub mesh: Option<UploadedMesh>,
    /// Where to draw the tool marker (work coordinates, mm).
    pub tool: Option<Vec3>,
}

impl egui_wgpu::CallbackTrait for VisualizerCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<GpuResources>() else {
            return Vec::new();
        };
        let uniforms = Uniforms {
            view_proj: self.view_proj.to_cols_array_2d(),
            done_color: self.done_color,
            progress: self.progress,
            srgb_target: u32::from(gpu.srgb_target),
            pad: [0; 2],
        };
        queue.write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));
        let generation = self.mesh.as_ref().map(|m| m.generation);
        if gpu.toolpath.as_ref().map(|t| t.0) != generation {
            gpu.toolpath = self.mesh.as_ref().map(|m| {
                let vertices = &m.mesh.vertices;
                let buffer = (!vertices.is_empty()).then(|| {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("gcodekit5_visualizer_toolpath"),
                        contents: bytemuck::cast_slice(vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    })
                });
                (m.generation, buffer, vertices.len() as u32)
            });
        }
        if let Some(tip) = self.tool {
            queue.write_buffer(&gpu.marker, 0, bytemuck::cast_slice(&marker_vertices(tip)));
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(gpu) = resources.get::<GpuResources>() else {
            return;
        };
        pass.set_bind_group(0, &gpu.bind_group, &[]);
        pass.set_pipeline(&gpu.line_pipeline);
        pass.set_vertex_buffer(0, gpu.scene.slice(..));
        pass.draw(0..gpu.scene_count, 0..1);
        if let Some((_, Some(buffer), count)) = &gpu.toolpath {
            pass.set_pipeline(&gpu.toolpath_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        if self.tool.is_some() {
            pass.set_pipeline(&gpu.marker_pipeline);
            pass.set_vertex_buffer(0, gpu.marker.slice(..));
            pass.draw(0..MARKER_VERTICES as u32, 0..1);
        }
    }
}
