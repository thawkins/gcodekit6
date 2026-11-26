//! The visualizer's wgpu drawing: pipelines, buffers and the egui paint callback.
//!
//! [`install`] builds the pipelines for the window's colour format once and keeps them in
//! the egui renderer's callback resources (as eframe's custom-3D pattern does). Each frame
//! the pane adds a [`VisualizerCallback`]; its `prepare` writes one small uniform (camera,
//! progress, which moves are shown) and uploads the toolpath only when a new program was
//! built, so a frame costs the same for a million-line program as for ten lines; the grid is
//! rebuilt only when its spacing changes. The upload takes the vertices out of
//! their [`Upload`], freeing the CPU copy; a toolpath larger than the device's
//! `max_buffer_size` is not uploaded ([`toolpath_fits`]) and the pane says so. Its `paint`
//! draws, in order: the grid (when shown), the origin axes, the machine bounds (when shown),
//! the toolpath (its travel and
//! its cuts each when shown, GTK's Show Rapid Moves and Show Cutting Moves), the tool
//! marker and the run preview's marker.
//!
//! The shaders are WGSL ports of the GTK `ui/gtk/shaders.rs` line and stock removal shaders.
//! The callback draws inside egui's render pass, which has no depth buffer, so later draws
//! cover earlier ones (the GTK view also draws the grid and the tool marker without depth
//! testing).
//!
//! The stock removal mesh ([`super::stock`]) must be depth-tested with the toolpath, as in
//! GTK. While one is shown, `prepare` draws the stock and then the toolpath into an offscreen
//! colour and depth target of the view's size, in its own pass; `paint` then copies that
//! image over the grid in place of the direct toolpath draw. The mesh is uploaded once per
//! run, and one larger than `max_buffer_size` is not drawn ([`stock_fits`]).
//!
//! The pipelines are built for egui's pass as eframe sets it up by default: no depth or
//! stencil attachment and one sample per pixel. Turning on `depth_buffer` or `multisampling`
//! in the app's `eframe::NativeOptions` (see `crate::run`) makes them incompatible with the
//! pass; change [`install`] with it.
//!
//! Dependencies: `egui-wgpu` (callback, wgpu), `bytemuck`, `glam`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};

use gcodekit5_visualizer::visualizer::stock_removal_3d::MESH_FLOATS_PER_VERTEX;

use super::camera::Aabb;
use super::stock::StockMesh;
use super::toolpath::ToolpathVertex;

/// Grid size (mm, centred on the origin), as GTK draws it.
const GRID_SIZE: f32 = 4000.0;
/// The grid spacing in mm until the sidebar changes it (GTK's default).
pub const DEFAULT_GRID_SPACING: f32 = 50.0;
/// Grid colour (GTK's 30% grey).
const GRID_COLOR: [u8; 4] = [77, 77, 77, 255];
/// Machine bounds colour (GTK's bright blue).
const BOUNDS_COLOR: [u8; 4] = [0, 128, 255, 255];
/// Vertices of the machine bounds box: its 12 edges.
const BOUNDS_VERTICES: usize = 24;
/// Length of the origin axes in mm.
const AXIS_LENGTH: f32 = 100.0;
/// Tool marker colour: red at 40% opacity, as GTK draws it.
const MARKER_COLOR: [u8; 4] = [255, 0, 0, 102];
/// Run preview marker colour: GTK's cyan (0, 0.85, 1) at 55% opacity.
pub const PREVIEW_MARKER_COLOR: [u8; 4] = [0, 217, 255, 140];
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

/// The shaders' uniform block. The line shaders declare only the fields up to `pad`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    done_color: [f32; 4],
    progress: u32,
    srgb_target: u32,
    /// Toolpath shader: [`SHOW_TRAVEL`] and [`SHOW_CUTS`].
    show: u32,
    pad: u32,
    /// Stock shader: the inverse transpose of the view's rotation (upper-left 3x3).
    normal_matrix: [[f32; 4]; 4],
    /// Stock shader: the light direction in view space.
    light_dir: [f32; 4],
}

/// The toolpath shader draws travel (rapids and moves with the spindle/laser off).
const SHOW_TRAVEL: u32 = 1;
/// The toolpath shader draws cuts.
const SHOW_CUTS: u32 = 2;

/// The stock shader's light direction in view space (GTK's `uLightDir`).
const STOCK_LIGHT_DIR: [f32; 4] = [0.35, 0.35, 1.0, 0.0];
/// Depth format of the offscreen stock pass.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Bytes per stock mesh vertex (position, normal, colour as `f32`).
const STOCK_STRIDE: u64 = (MESH_FLOATS_PER_VERTEX * std::mem::size_of::<f32>()) as u64;

/// The offscreen colour and depth target the stock and toolpath are drawn into.
struct Offscreen {
    size: [u32; 2],
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    /// Binds `color` for the composite shader.
    bind_group: wgpu::BindGroup,
}

/// The scene's GPU objects, kept in the egui renderer's callback resources.
struct GpuResources {
    toolpath_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    marker_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The grid, rebuilt when its spacing changes: buffer, vertex count and spacing.
    grid: (wgpu::Buffer, u32, f32),
    /// The origin axes, built once.
    axes: wgpu::Buffer,
    axes_count: u32,
    /// The tool marker, rewritten when the tool moves.
    marker: wgpu::Buffer,
    /// The run preview marker, rewritten as it moves.
    preview_marker: wgpu::Buffer,
    /// The machine bounds box, rewritten each frame it is shown.
    bounds: wgpu::Buffer,
    /// The uploaded toolpath: its generation, buffer and vertex count.
    toolpath: Option<(u64, Option<wgpu::Buffer>, u32)>,
    srgb_target: bool,
    /// The stock mesh, depth-tested and back-face culled.
    stock_pipeline: wgpu::RenderPipeline,
    /// The toolpath, depth-tested against the stock (offscreen).
    toolpath_depth_pipeline: wgpu::RenderPipeline,
    /// The axes and machine bounds, depth-tested against the stock in the offscreen pass,
    /// so the stock hides only the edges behind it.
    line_depth_pipeline: wgpu::RenderPipeline,
    /// Copies the offscreen image into egui's pass.
    composite_pipeline: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    /// The uploaded stock mesh: its generation, buffer and vertex count.
    stock: Option<(u64, Option<wgpu::Buffer>, u32)>,
    offscreen: Option<Offscreen>,
    format: wgpu::TextureFormat,
}

/// Pushes the line `a` to `b` in `color` onto `v`.
fn push_line(v: &mut Vec<SceneVertex>, a: Vec3, b: Vec3, color: [u8; 4]) {
    v.push(SceneVertex {
        pos: a.to_array(),
        color,
    });
    v.push(SceneVertex {
        pos: b.to_array(),
        color,
    });
}

/// The grid lines at Z = 0, `step` mm apart (GTK's `generate_grid_data`); a step under
/// 1 mm is taken as 1 mm.
///
/// # Arguments
/// * `step` - The spacing in mm.
pub fn grid_vertices(step: f32) -> Vec<SceneVertex> {
    let step = if step.is_finite() { step.max(1.0) } else { 1.0 };
    let mut v = Vec::new();
    let half = GRID_SIZE / 2.0;
    let steps = (GRID_SIZE / step) as i32;
    for i in 0..=steps {
        let p = -half + i as f32 * step;
        push_line(
            &mut v,
            Vec3::new(-half, p, 0.0),
            Vec3::new(half, p, 0.0),
            GRID_COLOR,
        );
        push_line(
            &mut v,
            Vec3::new(p, -half, 0.0),
            Vec3::new(p, half, 0.0),
            GRID_COLOR,
        );
    }
    v
}

/// The 12 edges of `bounds` (GTK's `generate_bounds_data`).
///
/// # Arguments
/// * `bounds` - The machine's work area in mm.
pub fn bounds_vertices(bounds: Aabb) -> Vec<SceneVertex> {
    let c = bounds.corners();
    // Corner index bits: 1 = max X, 2 = max Y, 4 = max Z.
    let edges = [
        (0, 1),
        (1, 3),
        (3, 2),
        (2, 0),
        (4, 5),
        (5, 7),
        (7, 6),
        (6, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let mut v = Vec::with_capacity(BOUNDS_VERTICES);
    for (a, b) in edges {
        push_line(&mut v, c[a], c[b], BOUNDS_COLOR);
    }
    v
}

/// The X, Y and Z axes from the origin.
fn axis_vertices() -> Vec<SceneVertex> {
    let mut v = Vec::new();
    let axes = [
        (Vec3::X, [255, 0, 0, 255]),
        (Vec3::Y, [0, 255, 0, 255]),
        (Vec3::Z, [0, 0, 255, 255]),
    ];
    for (axis, color) in axes {
        push_line(&mut v, Vec3::ZERO, axis * AXIS_LENGTH, color);
    }
    v
}

/// A vertex buffer holding `vertices`.
fn scene_buffer(device: &wgpu::Device, vertices: &[SceneVertex]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("gcodekit5_visualizer_scene"),
        contents: bytemuck::cast_slice(vertices),
        usage: wgpu::BufferUsages::VERTEX,
    })
}

/// A tool marker: a cone with its tip at `tip` and its base 10 mm above, in `color`.
fn marker_vertices(tip: Vec3, color: [u8; 4]) -> Vec<SceneVertex> {
    let base = tip + Vec3::Z * MARKER_HEIGHT;
    let rim = |i: usize| {
        let a = i as f32 / MARKER_SIDES as f32 * std::f32::consts::TAU;
        base + Vec3::new(MARKER_RADIUS * a.cos(), MARKER_RADIUS * a.sin(), 0.0)
    };
    let vertex = |p: Vec3| SceneVertex {
        pos: p.to_array(),
        color,
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

/// How a pipeline draws: its vertex layout, primitives and depth test.
struct Draw<'a> {
    vertex: Option<wgpu::VertexBufferLayout<'a>>,
    primitive: wgpu::PrimitiveState,
    depth: Option<wgpu::DepthStencilState>,
}

impl<'a> Draw<'a> {
    /// `topology` from `vertex`, without depth (egui's pass).
    fn plain(vertex: wgpu::VertexBufferLayout<'a>, topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            vertex: Some(vertex),
            primitive: wgpu::PrimitiveState {
                topology,
                ..wgpu::PrimitiveState::default()
            },
            depth: None,
        }
    }
}

/// A render pipeline drawing `draw` with `shader`'s `vs_main` and `fs_main`.
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    draw: Draw<'_>,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("gcodekit5_visualizer"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[draw.vertex],
        },
        primitive: draw.primitive,
        depth_stencil: draw.depth,
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
    let grid = grid_vertices(DEFAULT_GRID_SPACING);
    let axes = axis_vertices();
    let stock_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/stock.wgsl"));
    let composite_shader =
        device.create_shader_module(wgpu::include_wgsl!("shaders/composite.wgsl"));
    let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gcodekit5_visualizer_composite_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        }],
    });
    let composite_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gcodekit5_visualizer_composite"),
            bind_group_layouts: &[Some(&composite_layout)],
            immediate_size: 0,
        });
    let stock_layout = wgpu::VertexBufferLayout {
        array_stride: STOCK_STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
    };
    // GTK culls back faces (counter-clockwise front) and tests with LESS. The bias pushes the
    // stock back a little, so toolpath lines on a cut floor are not lost to depth fighting.
    let stock_draw = Draw {
        vertex: Some(stock_layout),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..wgpu::PrimitiveState::default()
        },
        depth: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
    };
    // Lines are tested against the stock but do not write depth, so their order is free.
    let line_depth = wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    };
    let toolpath_depth_draw = Draw {
        depth: Some(line_depth.clone()),
        ..Draw::plain(toolpath_layout.clone(), lines)
    };
    let line_depth_draw = Draw {
        depth: Some(line_depth),
        ..Draw::plain(scene_layout.clone(), lines)
    };
    let composite_draw = Draw {
        vertex: None,
        primitive: wgpu::PrimitiveState::default(),
        depth: None,
    };
    let resources = GpuResources {
        toolpath_pipeline: pipeline(
            device,
            &layout,
            &toolpath_shader,
            Draw::plain(toolpath_layout, lines),
            format,
        ),
        line_pipeline: pipeline(
            device,
            &layout,
            &grid_shader,
            Draw::plain(scene_layout.clone(), lines),
            format,
        ),
        marker_pipeline: pipeline(
            device,
            &layout,
            &grid_shader,
            Draw::plain(scene_layout, wgpu::PrimitiveTopology::TriangleList),
            format,
        ),
        stock_pipeline: pipeline(device, &layout, &stock_shader, stock_draw, format),
        toolpath_depth_pipeline: pipeline(
            device,
            &layout,
            &toolpath_shader,
            toolpath_depth_draw,
            format,
        ),
        line_depth_pipeline: pipeline(device, &layout, &grid_shader, line_depth_draw, format),
        composite_pipeline: pipeline(
            device,
            &composite_pipeline_layout,
            &composite_shader,
            composite_draw,
            format,
        ),
        composite_layout,
        stock: None,
        offscreen: None,
        format,
        uniforms,
        bind_group,
        grid: (
            scene_buffer(device, &grid),
            grid.len() as u32,
            DEFAULT_GRID_SPACING,
        ),
        axes: scene_buffer(device, &axes),
        axes_count: axes.len() as u32,
        marker: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gcodekit5_visualizer_marker"),
            size: (MARKER_VERTICES * std::mem::size_of::<SceneVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        preview_marker: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gcodekit5_visualizer_preview_marker"),
            size: (MARKER_VERTICES * std::mem::size_of::<SceneVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        bounds: device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gcodekit5_visualizer_bounds"),
            size: (BOUNDS_VERTICES * std::mem::size_of::<SceneVertex>()) as u64,
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

/// Whether `vertex_count` toolpath vertices fit in one vertex buffer of at most
/// `max_buffer_size` bytes (the device limit) and in a `u32` draw range.
///
/// # Arguments
/// * `vertex_count` - Vertices to upload.
/// * `max_buffer_size` - The device's `max_buffer_size` limit in bytes.
pub fn toolpath_fits(vertex_count: usize, max_buffer_size: u64) -> bool {
    let stride = std::mem::size_of::<ToolpathVertex>() as u64;
    u32::try_from(vertex_count).is_ok()
        && (vertex_count as u64)
            .checked_mul(stride)
            .is_some_and(|bytes| bytes <= max_buffer_size)
}

/// Whether a stock mesh of `floats` floats fits in one vertex buffer of at most
/// `max_buffer_size` bytes (the device limit) and in a `u32` draw range.
///
/// # Arguments
/// * `floats` - The mesh's length in floats.
/// * `max_buffer_size` - The device's `max_buffer_size` limit in bytes.
pub fn stock_fits(floats: usize, max_buffer_size: u64) -> bool {
    u32::try_from(floats / MESH_FLOATS_PER_VERTEX).is_ok()
        && (floats as u64)
            .checked_mul(std::mem::size_of::<f32>() as u64)
            .is_some_and(|bytes| bytes <= max_buffer_size)
}

/// Vertices waiting for the GPU (a toolpath's, or a stock mesh's floats). The callback takes
/// them when it uploads, so the CPU copy is freed once it is on the GPU.
#[derive(Debug)]
pub struct Upload<T = ToolpathVertex> {
    vertices: Mutex<Option<Vec<T>>>,
    too_large: AtomicBool,
}

impl<T> Default for Upload<T> {
    fn default() -> Self {
        Self {
            vertices: Mutex::new(None),
            too_large: AtomicBool::new(false),
        }
    }
}

impl<T> Upload<T> {
    /// Vertices to upload.
    ///
    /// # Arguments
    /// * `vertices` - The vertices (or floats) to upload.
    pub fn new(vertices: Vec<T>) -> Self {
        Self {
            vertices: Mutex::new(Some(vertices)),
            too_large: AtomicBool::new(false),
        }
    }

    /// Whether the vertices are still on the CPU (not uploaded yet).
    pub fn pending(&self) -> bool {
        self.lock().is_some()
    }

    /// A copy of the vertices still waiting for the GPU, if not uploaded yet.
    pub fn snapshot(&self) -> Option<Vec<T>>
    where
        T: Clone,
    {
        self.lock().clone()
    }

    /// Whether the toolpath was too large for the GPU's buffer limit and is not drawn.
    pub fn too_large(&self) -> bool {
        self.too_large.load(Ordering::Relaxed)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Vec<T>>> {
        self.vertices.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A toolpath built for one program, tagged so it is uploaded once.
#[derive(Debug, Clone)]
pub struct UploadedMesh {
    /// Changes whenever a new program's mesh is built.
    pub generation: u64,
    /// The vertices, until uploaded.
    pub upload: Arc<Upload>,
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
    /// Where to draw the run preview marker (mm).
    pub preview: Option<Vec3>,
    /// The camera's view matrix, to light the stock in view space.
    pub view: Mat4,
    /// The stock removal mesh, when the simulation is shown and finished.
    pub stock: Option<StockMesh>,
    /// The view's rectangle in points, to size the offscreen target.
    pub rect: egui::Rect,
    /// Whether travel (rapids and moves with the spindle/laser off) is drawn.
    pub show_travel: bool,
    /// Whether cuts are drawn.
    pub show_cuts: bool,
    /// The grid's spacing in mm, or `None` to leave the grid out.
    pub grid: Option<f32>,
    /// The machine's work area to outline, or `None` to leave it out.
    pub machine_bounds: Option<Aabb>,
}

/// The view's size in physical pixels, as egui sets the callback's viewport (rounded per
/// edge and clamped to the screen), at least 1 and at most `max_dimension` (the device's
/// `max_texture_dimension_2d`).
///
/// # Arguments
/// * `rect` - The view's rectangle in points.
/// * `screen` - The frame's screen size and scale.
/// * `max_dimension` - The largest texture side the device allows.
pub fn size_px(
    rect: egui::Rect,
    screen: &egui_wgpu::ScreenDescriptor,
    max_dimension: u32,
) -> [u32; 2] {
    let viewport = egui::epaint::ViewportInPixels::from_points(
        &rect,
        screen.pixels_per_point,
        screen.size_in_pixels,
    );
    let side = |px: i32| {
        u32::try_from(px)
            .unwrap_or(0)
            .clamp(1, max_dimension.max(1))
    };
    [side(viewport.width_px), side(viewport.height_px)]
}

impl GpuResources {
    /// Uploads `stock` when it is a new run's mesh, or drops the old one.
    fn upload_stock(&mut self, device: &wgpu::Device, stock: Option<&StockMesh>) {
        let generation = stock.map(|m| m.generation);
        if self.stock.as_ref().map(|s| s.0) == generation {
            return;
        }
        self.stock = stock.map(|m| {
            let floats = m.upload.lock().take().unwrap_or_default();
            let limit = device.limits().max_buffer_size;
            if !stock_fits(floats.len(), limit) {
                tracing::warn!(
                    floats = floats.len(),
                    limit,
                    "stock mesh exceeds the GPU buffer limit; not drawn"
                );
                m.upload.too_large.store(true, Ordering::Relaxed);
                return (m.generation, None, 0);
            }
            let buffer = (!floats.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("gcodekit5_visualizer_stock"),
                    contents: bytemuck::cast_slice(&floats),
                    usage: wgpu::BufferUsages::VERTEX,
                })
            });
            // stock_fits checked the count fits in u32.
            let count = u32::try_from(floats.len() / MESH_FLOATS_PER_VERTEX).unwrap_or(0);
            (m.generation, buffer, count)
        });
    }

    /// (Re)makes the offscreen target at `size`.
    fn ensure_offscreen(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.offscreen.as_ref().is_some_and(|o| o.size == size) {
            return;
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let texture = |label, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let color = texture(
            "gcodekit5_visualizer_offscreen",
            self.format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture(
            "gcodekit5_visualizer_depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gcodekit5_visualizer_composite"),
            layout: &self.composite_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&color),
            }],
        });
        self.offscreen = Some(Offscreen {
            size,
            color,
            depth,
            bind_group,
        });
    }

    /// Draws the stock and then the axes, the machine bounds (when `bounds`) and the
    /// toolpath, depth-tested, into the offscreen target; with no stock to draw, releases
    /// the target.
    fn draw_offscreen(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size: [u32; 2],
        bounds: bool,
    ) {
        let Some((_, Some(_), _)) = &self.stock else {
            self.offscreen = None;
            return;
        };
        self.ensure_offscreen(device, size);
        let (Some(offscreen), Some((_, Some(stock), stock_count))) = (&self.offscreen, &self.stock)
        else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gcodekit5_visualizer_stock_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &offscreen.color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &offscreen.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.stock_pipeline);
        pass.set_vertex_buffer(0, stock.slice(..));
        pass.draw(0..*stock_count, 0..1);
        pass.set_pipeline(&self.line_depth_pipeline);
        pass.set_vertex_buffer(0, self.axes.slice(..));
        pass.draw(0..self.axes_count, 0..1);
        if bounds {
            pass.set_vertex_buffer(0, self.bounds.slice(..));
            pass.draw(0..BOUNDS_VERTICES as u32, 0..1);
        }
        if let Some((_, Some(buffer), count)) = &self.toolpath {
            pass.set_pipeline(&self.toolpath_depth_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
    }

    /// Whether the stock is drawn this frame (offscreen, with the toolpath).
    fn stock_drawn(&self) -> bool {
        matches!(&self.stock, Some((_, Some(_), _))) && self.offscreen.is_some()
    }
}

impl egui_wgpu::CallbackTrait for VisualizerCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
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
            show: u32::from(self.show_travel) * SHOW_TRAVEL + u32::from(self.show_cuts) * SHOW_CUTS,
            pad: 0,
            normal_matrix: Mat4::from_mat3(glam::Mat3::from_mat4(self.view).inverse().transpose())
                .to_cols_array_2d(),
            light_dir: STOCK_LIGHT_DIR,
        };
        queue.write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));
        if let Some(step) = self.grid
            && step != gpu.grid.2
        {
            let grid = grid_vertices(step);
            gpu.grid = (scene_buffer(device, &grid), grid.len() as u32, step);
        }
        let generation = self.mesh.as_ref().map(|m| m.generation);
        if gpu.toolpath.as_ref().map(|t| t.0) != generation {
            gpu.toolpath = self.mesh.as_ref().map(|m| {
                let vertices = m.upload.lock().take().unwrap_or_default();
                let limit = device.limits().max_buffer_size;
                if !toolpath_fits(vertices.len(), limit) {
                    tracing::warn!(
                        vertices = vertices.len(),
                        limit,
                        "toolpath exceeds the GPU buffer limit; not drawn"
                    );
                    m.upload.too_large.store(true, Ordering::Relaxed);
                    return (m.generation, None, 0);
                }
                let buffer = (!vertices.is_empty()).then(|| {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("gcodekit5_visualizer_toolpath"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    })
                });
                // toolpath_fits checked the count fits in u32.
                let count = u32::try_from(vertices.len()).unwrap_or(0);
                (m.generation, buffer, count)
            });
        }
        if let Some(bounds) = self.machine_bounds {
            let vertices = bounds_vertices(bounds);
            queue.write_buffer(&gpu.bounds, 0, bytemuck::cast_slice(&vertices));
        }
        if let Some(tip) = self.tool {
            let vertices = marker_vertices(tip, MARKER_COLOR);
            queue.write_buffer(&gpu.marker, 0, bytemuck::cast_slice(&vertices));
        }
        if let Some(tip) = self.preview {
            let vertices = marker_vertices(tip, PREVIEW_MARKER_COLOR);
            queue.write_buffer(&gpu.preview_marker, 0, bytemuck::cast_slice(&vertices));
        }
        gpu.upload_stock(device, self.stock.as_ref());
        let size = size_px(self.rect, screen, device.limits().max_texture_dimension_2d);
        gpu.draw_offscreen(device, encoder, size, self.machine_bounds.is_some());
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
        if self.grid.is_some() {
            pass.set_vertex_buffer(0, gpu.grid.0.slice(..));
            pass.draw(0..gpu.grid.1, 0..1);
        }
        // With the stock, the axes and bounds are in the offscreen pass, depth-tested
        // against it; the grid stays under the stock.
        let offscreen = gpu.offscreen.as_ref().filter(|_| gpu.stock_drawn());
        if offscreen.is_none() {
            pass.set_vertex_buffer(0, gpu.axes.slice(..));
            pass.draw(0..gpu.axes_count, 0..1);
            if self.machine_bounds.is_some() {
                pass.set_vertex_buffer(0, gpu.bounds.slice(..));
                pass.draw(0..BOUNDS_VERTICES as u32, 0..1);
            }
        }
        if let Some(offscreen) = offscreen {
            pass.set_pipeline(&gpu.composite_pipeline);
            pass.set_bind_group(0, &offscreen.bind_group, &[]);
            pass.draw(0..3, 0..1);
            pass.set_bind_group(0, &gpu.bind_group, &[]);
        } else if let Some((_, Some(buffer), count)) = &gpu.toolpath {
            pass.set_pipeline(&gpu.toolpath_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..*count, 0..1);
        }
        if self.tool.is_some() {
            pass.set_pipeline(&gpu.marker_pipeline);
            pass.set_vertex_buffer(0, gpu.marker.slice(..));
            pass.draw(0..MARKER_VERTICES as u32, 0..1);
        }
        if self.preview.is_some() {
            pass.set_pipeline(&gpu.marker_pipeline);
            pass.set_vertex_buffer(0, gpu.preview_marker.slice(..));
            pass.draw(0..MARKER_VERTICES as u32, 0..1);
        }
    }
}
