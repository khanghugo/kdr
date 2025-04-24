//! Taken from https://github.com/kaphula/winit-egui-wgpu-template/blob/master/src/egui_tools.rs

use tracing::warn;

pub struct EguiRenderer {
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    frame_started: bool,
}

impl EguiRenderer {
    pub fn context(&self) -> &egui::Context {
        self.state.egui_ctx()
    }

    pub fn new(
        device: &wgpu::Device,
        output_color_format: wgpu::TextureFormat,
        output_depth_format: Option<wgpu::TextureFormat>,
        msaa_samples: u32,
        window: &winit::window::Window,
    ) -> Self {
        let egui_ctx = egui::Context::default();

        // add fonts
        {
            let verdana_bytes = include_bytes!("../../../fonts/Verdana.ttf");
            let tahoma_bytes = include_bytes!("../../../fonts/tahoma.ttf");
            // for chinese glyphs
            let jhenghei_bytes = include_bytes!("../../../fonts/microsoft-jhenghei.ttf");

            let mut fonts = egui::FontDefinitions::default();

            fonts.font_data.insert(
                "verdana".to_string(),
                egui::FontData::from_static(verdana_bytes).into(),
            );
            fonts.font_data.insert(
                "tahoma".to_string(),
                egui::FontData::from_static(tahoma_bytes).into(),
            );
            fonts.font_data.insert(
                "jhenghei".to_string(),
                egui::FontData::from_static(jhenghei_bytes).into(),
            );

            let tahoma_family = fonts
                .families
                .entry(egui::FontFamily::Name("tahoma".into()))
                .or_default();

            tahoma_family.push("tahoma".into());
            tahoma_family.push("jhenghei".into());

            let verdana_family = fonts
                .families
                .entry(egui::FontFamily::Name("verdana".into()))
                .or_default();

            verdana_family.push("verdana".into());
            verdana_family.push("jhenghei".into());

            // fallback for general egui texts
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("jhenghei".into());

            egui_ctx.set_fonts(fonts);
        }

        let state = egui_winit::State::new(
            egui_ctx,
            egui::viewport::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(2 * 1024),
        );

        let renderer = egui_wgpu::Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
            true,
        );

        Self {
            state,
            renderer,
            frame_started: false,
        }
    }

    fn ppp(&mut self, v: f32) {
        self.context().set_pixels_per_point(v);
    }

    pub fn handle_input(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn begin_frame(&mut self, window: &winit::window::Window) {
        let raw_input = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
    ) {
        if !self.frame_started {
            warn!("begin_frame must be called before end_frame_and_draw can be called!");
            return;
        }

        // self.ppp(screen_descriptor.pixels_per_point);

        let full_output = self.state.egui_ctx().end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .egui_ctx()
            .tessellate(full_output.shapes, self.state.egui_ctx().pixels_per_point());

        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui ui pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);

        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }

        self.frame_started = false;
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        window: &winit::window::Window,
        window_surface_view: &wgpu::TextureView,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
        mut draw_function: impl FnMut(&egui::Context) -> (),
    ) {
        self.begin_frame(window);

        // draw stuffs code
        draw_function(self.context());

        self.end_frame_and_draw(
            device,
            queue,
            encoder,
            window,
            window_surface_view,
            screen_descriptor,
        );
    }
}
