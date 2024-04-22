use std::sync::Arc;
use camera::{ GenericCamera, PerspectiveCamera};
use controller::CameraController;
#[cfg(target_arch = "wasm32")]
use instant::{Duration, Instant};
use renderer::{RenderSettings, VolumeRenderer};
use volume::VolumeGPU;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};
use wgpu::Backends;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;


use cgmath::{
    vec3, Deg, EuclideanSpace, InnerSpace,  Point3, Quaternion, Rotation, Vector2, Vector3
};
use egui::Color32;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;
use winit::{
    dpi::PhysicalSize, event::{DeviceEvent, ElementState, Event, WindowEvent}, event_loop::EventLoop, keyboard::{KeyCode, PhysicalKey}, window::{Window, WindowBuilder}
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowBuilderExtWebSys;

use crate::{
    camera::PerspectiveProjection,
    cmap::{ColorMap, ColorMapGPU},
    volume::{Aabb, Volume},
};

mod camera;
pub mod cmap;
mod controller;
mod renderer;
mod ui;
mod ui_renderer;
pub mod volume;

pub struct RenderConfig {
    pub no_vsync: bool,
}

pub struct WGPUContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
}

impl WGPUContext {

    pub async fn new(instance: &wgpu::Instance, surface: Option<&wgpu::Surface<'static>>) -> Self {
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, surface)
            .await
            .unwrap();

        let required_features = wgpu::Features::default();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features,
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        Self {
            device,
            queue,
            adapter,
        }
    }
}

pub struct WindowContext {
    wgpu_context: WGPUContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    scale_factor: f32,

    controller: CameraController,
    camera: GenericCamera<PerspectiveProjection>,
    ui_renderer: ui_renderer::EguiWGPU,
    ui_visible: bool,

    background_color: egui::Color32,

    volumes: Vec<VolumeGPU>,
    renderer: VolumeRenderer,

    render_settings: RenderSettings,
    cmap: cmap::ColorMapGPU,

    playing:bool,
    animation_duration:Duration,
    num_columns:u32,
    selected_channel:Option<usize>,
}

impl WindowContext {
    // Creating some of the wgpu types requires async code
    async fn new(
        window: Window,
        volumes: Vec<Volume>,
        cmap:ColorMap,
        render_config: &RenderConfig,
    ) -> anyhow::Result<Self> {
        let mut size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            size = PhysicalSize::new(800, 600);
        }
        let window = Arc::new(window);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor{
             backends: Backends::all().symmetric_difference(Backends::BROWSER_WEBGPU), 
             ..Default::default() });

        let surface: wgpu::Surface = instance.create_surface(window.clone())?;

        let wgpu_context = WGPUContext::new(&instance, Some(&surface)).await;

        log::info!("device: {:?}", wgpu_context.adapter.get_info().name);

        let device = &wgpu_context.device;
        let queue = &wgpu_context.queue;

        let max_size = device.limits().max_texture_dimension_2d;
        window.set_max_inner_size(Some(PhysicalSize::new(max_size,max_size)));

        let surface_caps = surface.get_capabilities(&wgpu_context.adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(&surface_caps.formats[0])
            .clone();
        let surface_format = surface_format;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            desired_maximum_frame_latency: 2,
            present_mode: if render_config.no_vsync {
                wgpu::PresentMode::AutoNoVsync
            } else {
                wgpu::PresentMode::AutoVsync
            },
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let ui_renderer = ui_renderer::EguiWGPU::new(device, surface_format, &window);

        let renderer = VolumeRenderer::new(device, surface_format);

        let render_settings = RenderSettings {
            clipping_aabb: Aabb::unit(),
            time: 0.,
            step_size: 2. / 1000.,
            spatial_filter: wgpu::FilterMode::Linear,
            temporal_filter: wgpu::FilterMode::Linear,
            distance_scale: 1.,
            vmin:volumes[0].min_value,
            vmax:volumes[0].max_value,
        };

        let mut controller = CameraController::new(0.1, 0.05);
        controller.center = volumes[0].aabb.center();
        let r = volumes[0].aabb.radius();
        let corner = vec3(1., -1., 1.);
        let view_dir = Quaternion::look_at(-corner, Vector3::unit_y());
        let camera = PerspectiveCamera::new(
            Point3::from_vec(corner.normalize()) * r * 3.,
            view_dir,
            PerspectiveProjection::new(
                Vector2::new(size.width, size.height),
                Deg(45.),
                0.01,
                1000.,
            ),
        );

        let animation_duration = Duration::from_secs_f32(volumes[0].timesteps as f32*0.05);
        
        let num_columns =  volumes.len().min(4) as u32;
        let volumes_gpu = volumes.into_iter().map(|v| VolumeGPU::new(device, queue, v)).collect();

        let cmap = ColorMapGPU::new(cmap, device, queue);
        Ok(Self {   
            wgpu_context,
            scale_factor: window.scale_factor() as f32,
            window,
            surface,
            config,
            controller,
            ui_renderer,
            ui_visible: true,
            background_color: Color32::BLACK,
            camera,

            volumes:volumes_gpu,
            renderer,
            render_settings,
            cmap,
            animation_duration,
            playing: true,
            num_columns,
            selected_channel:None,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>, scale_factor: Option<f32>) {
        if new_size.width > 0 && new_size.height > 0 {
            let new_width = new_size.width;
            let new_height = new_size.height;
            self.config.width = new_width;
            self.config.height = new_height;
            self.camera.projection.resize(new_width, new_height);
            self.surface
                .configure(&self.wgpu_context.device, &self.config);
        }
        if let Some(scale_factor) = scale_factor {
            if scale_factor > 0. {
                self.scale_factor = scale_factor;
            }
        }
    }

    fn update(&mut self, dt: Duration) {
        self.controller.update_camera(&mut self.camera, dt);
        
        if self.playing && self.volumes[0].volume.timesteps > 1{
            self.render_settings.time += dt.as_secs_f32() / self.animation_duration.as_secs_f32();
            self.render_settings.time = self.render_settings.time.fract();
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let window_size = self.window.inner_size();
        if window_size.width != self.config.width || window_size.height != self.config.height {
            self.resize(window_size, None);
        }

        let output = self.surface.get_current_texture()?;
        let view_rgb = output.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format),
            ..Default::default()
        });

        // do prepare stuff

        let mut encoder =
            self.wgpu_context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render command encoder"),
                });
        let mut frame_data = Vec::new();

        let columns = self.num_columns as usize;
        let rows = (self.volumes.len() as f32 / columns as f32).ceil() as usize;
        let cell_width = self.config.width as f32 / columns as f32;
        let cell_height = self.config.height as f32 / rows as f32;

        if let Some(selected_channel) = self.selected_channel{
            let camera = self.camera.clone();
            frame_data.push(self.renderer.prepare(
                &self.wgpu_context.device,
                &self.volumes[selected_channel],
                &camera,
                &self.render_settings,
                &self.cmap
            ));
        }else{
            for v in &self.volumes{
                let mut camera = self.camera.clone();
                camera.projection.resize(cell_width as u32, cell_height as u32);
                frame_data.push(self.renderer.prepare(
                    &self.wgpu_context.device,
                    &v,
                    &camera,
                    &self.render_settings,
                    &self.cmap
                ));
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_rgb,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background_color.r() as f64 / 255.,
                            g: self.background_color.g() as f64 / 255.,
                            b: self.background_color.b() as f64 / 255.,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            for (i,v) in frame_data.iter().enumerate(){
                if self.selected_channel.is_none(){
                    let column = i % columns;
                    let row = i / columns;
                    render_pass.set_viewport(
                        column as f32 * cell_width, 
                        row as f32 * cell_height,
                        cell_width,
                        cell_height, 
                        0., 1.);
                }
                self.renderer
                    .render(&mut render_pass,  &v);
            }
        }

        self.wgpu_context
            .queue
            .submit(std::iter::once(encoder.finish()));

        if self.ui_visible {
            // ui rendering
            self.ui_renderer.begin_frame(&self.window);
            ui::ui(self);

            let shapes = self.ui_renderer.end_frame(&self.window);

            self.ui_renderer.paint(
                PhysicalSize {
                    width: output.texture.size().width,
                    height: output.texture.size().height,
                },
                self.scale_factor,
                &self.wgpu_context.device,
                &self.wgpu_context.queue,
                &view_rgb,
                shapes,
            );
        }

        output.present();
        Ok(())
    }
}



pub async fn open_window(window_builder:WindowBuilder,volumes: Vec<Volume>,cmap: ColorMap,config: RenderConfig) {
    let event_loop = EventLoop::new().unwrap();


    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    let window = window_builder
        .with_title(format!("{name} {version}"))
        .build(&event_loop)
        .unwrap();
    let mut state = WindowContext::new(window, volumes, cmap,&config).await.unwrap();


    let mut last = Instant::now();

    event_loop.run(move |event,target| 
        
        match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window.id() && !state.ui_renderer.on_event(&state.window,event) => match event {
            WindowEvent::Resized(physical_size) => {
                state.resize(*physical_size, None);
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                ..
            } => {
                state.scale_factor = *scale_factor as f32;
            }
            WindowEvent::CloseRequested => {log::info!("close!");target.exit()},
            WindowEvent::ModifiersChanged(m)=>{
                state.controller.alt_pressed = m.state().alt_key();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key{
                    state
                        .controller
                        .process_keyboard(key, event.state == ElementState::Pressed);
                    if key == KeyCode::KeyU && event.state == ElementState::Released{
                        state.ui_visible = !state.ui_visible;
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                winit::event::MouseScrollDelta::LineDelta(_, dy) => {
                    state.controller.process_scroll(*dy )
                }
                winit::event::MouseScrollDelta::PixelDelta(p) => {
                    state.controller.process_scroll(p.y as f32 / 100.)
                }
            },
            WindowEvent::MouseInput { state:button_state, button, .. }=>{
                match button {
                    winit::event::MouseButton::Left =>                         state.controller.left_mouse_pressed = *button_state == ElementState::Pressed,
                    winit::event::MouseButton::Right => state.controller.right_mouse_pressed = *button_state == ElementState::Pressed,
                    _=>{}
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now-last;
                last = now;
                state.update(dt);
    
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.window.inner_size(), None),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) =>target.exit(),
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => println!("error: {:?}", e),
                }
            }
            _ => {}
        },
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion{ delta, },
            .. // We're not using device_id currently
        } => {
            state.controller.process_mouse(delta.0 as f32, delta.1 as f32)
        }
        
        Event::AboutToWait => {
            // RedrawRequested will only trigger once, unless we manually
            // request it.
            state.window.request_redraw();
        }
        _ => {},
    }).unwrap();
}



#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn viewer_inline(npz_file:Vec<u8>,colormap:Vec<u8>,canvas_id:String) {
    use std::io::Cursor;
    #[cfg(debug_assertions)]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("could not initialize logger");
    let reader = Cursor::new(npz_file);
    let volumes = Volume::load_npy(reader,true).expect("Failed to load volumes");
    
    let reader_colormap = Cursor::new(colormap);
    let cmap = ColorMap::from_npy(reader_colormap).unwrap();

    let canvas = web_sys::window()
    .and_then(|win| win.document())
    .and_then(|doc| {
        doc.get_element_by_id(&canvas_id).unwrap().dyn_into::<web_sys::HtmlCanvasElement>().ok()
    });
    let window_builder = WindowBuilder::new().with_canvas(canvas);

    wasm_bindgen_futures::spawn_local(open_window(
        window_builder,
        volumes,
        cmap,
        RenderConfig {
            no_vsync: false,
        },
    ));
}


#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn viewer_wasm(canvas_id:String) {
    use std::io::Cursor;

    use web_sys::{HtmlCanvasElement, HtmlElement};
    #[cfg(debug_assertions)]
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init().expect("could not initialize logger");
   
    
    let cmap = cmap::COLORMAPS.get("viridis").unwrap().clone();

    let (canvas,spinner):(HtmlCanvasElement,HtmlElement) = web_sys::window()
    .and_then(|win| win.document())
    .and_then(|doc| {
        let canvas = doc.get_element_by_id(&canvas_id).unwrap().dyn_into::<web_sys::HtmlCanvasElement>().unwrap();
        let spinner = doc.get_element_by_id("spinner").unwrap().dyn_into::<web_sys::HtmlElement>().unwrap();
        Some((canvas,spinner))
    }).unwrap();
    let size = (canvas.width() as u32,canvas.height() as u32);
    let window_builder = WindowBuilder::new().with_canvas(Some(canvas)).with_inner_size(PhysicalSize::new(size.0, size.1));

    loop {
        if let Some(reader) = rfd::AsyncFileDialog::new().set_title("Select npy file").add_filter("numpy file", &["npy","npz"]).pick_file().await {

            spinner.set_attribute("style", "display:flex;")
            .unwrap();
            let data = reader.read().await;
            let is_npz = data.starts_with(b"\x50\x4B\x03\x04");
            let reader_v = Cursor::new(data);
            let volumes = if is_npz{Volume::load_npz(reader_v,true) }else{Volume::load_npy(reader_v,true)}.expect("Failed to load volumes");

            spinner.set_attribute("style", "display:none;")
            .unwrap();
            wasm_bindgen_futures::spawn_local(open_window(
                window_builder,
                volumes,
                cmap,
                RenderConfig {
                    no_vsync: false,
                },
            ));
            break;
        }
    }
}
