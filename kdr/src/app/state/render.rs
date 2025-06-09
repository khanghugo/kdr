use crate::renderer::{
    RenderContext,
    camera::Camera,
    skybox::SkyboxBuffer,
    world_buffer::{
        PushConstantRenderFlags, WorldDynamicBuffer, WorldPushConstants, WorldStaticBuffer,
    },
};

use super::AppState;

pub struct RenderState {
    pub world_buffer: Option<WorldStaticBuffer>,
    pub skybox: Option<SkyboxBuffer>,

    pub viewmodel_buffers: Vec<WorldDynamicBuffer>,
    pub playermodel_buffers: Vec<WorldDynamicBuffer>,

    pub camera: Camera,
    pub render_options: RenderOptions,

    // debug
    pub draw_call: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            skybox: None,
            draw_call: 0,
            world_buffer: None,
            render_options: RenderOptions::default(),
            viewmodel_buffers: vec![],
            playermodel_buffers: vec![],
        }
    }
}

#[derive(Clone, Copy)]
pub struct RenderOptions {
    pub render_nodraw: bool,
    // TODO, eh, make it better?
    pub render_beyond_sky: bool,
    pub render_skybox: bool,
    pub render_transparent: bool,
    pub full_bright: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            render_nodraw: false,
            render_beyond_sky: false,
            render_skybox: true,
            render_transparent: true,
            full_bright: false,
        }
    }
}

impl AppState {
    pub fn render(
        &mut self,
        render_context: &RenderContext,
        encoder: &mut wgpu::CommandEncoder,
        swapchain_view: &wgpu::TextureView,
    ) {
        // update camera buffer
        {
            let view = self.render_state.camera.view();
            let view_cast: &[f32; 16] = view.as_ref();
            let view_bytes: &[u8] = bytemuck::cast_slice(view_cast);

            let proj = self.render_state.camera.proj();
            let proj_cast: &[f32; 16] = proj.as_ref();
            let proj_bytes: &[u8] = bytemuck::cast_slice(proj_cast);

            let pos = self.render_state.camera.pos;
            let pos_cast: &[f32; 3] = pos.as_ref();
            let pos_bytes: &[u8] = bytemuck::cast_slice(pos_cast);

            render_context
                .queue()
                .write_buffer(&render_context.camera_buffer.view, 0, view_bytes);
            render_context.queue().write_buffer(
                &render_context.camera_buffer.projection,
                0,
                proj_bytes,
            );
            render_context.queue().write_buffer(
                &render_context.camera_buffer.position,
                0,
                pos_bytes,
            );
        }

        self.render_state.draw_call = 0;

        // UPDATE: no more z pre pass, it is more troubling than it is worth it
        // the game doesn't have enough polygon to worry about overdrawing
        // on top of that, dealing with alpha test texture is not very fun
        // it might hurt more performance just to fix the alpha test texture depth
        //
        // z prepass
        // if true {
        //     let z_prepass_pass_descriptor = wgpu::RenderPassDescriptor {
        //         label: Some("world z prepass pass descriptor"),
        //         color_attachments: &[],
        //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
        //             view: &self.render_targets.depth_view,
        //             depth_ops: Some(wgpu::Operations {
        //                 load: wgpu::LoadOp::Clear(1.0),
        //                 store: wgpu::StoreOp::Store,
        //             }),
        //             stencil_ops: None,
        //         }),
        //         timestamp_writes: None,
        //         occlusion_query_set: None,
        //     };

        //     let mut z_prepass_pass = encoder.begin_render_pass(&z_prepass_pass_descriptor);

        //     z_prepass_pass.set_pipeline(&self.world_z_prepass_render_pipeline);
        //     z_prepass_pass.set_bind_group(0, &self.camera_buffer.bind_group, &[]);

        //     state.world_buffer.iter().for_each(|world_buffer| {
        //         z_prepass_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
        //         z_prepass_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

        //         world_buffer.opaque.iter().for_each(|batch| {
        //             // state.draw_call += 1;

        //             // texture array
        //             z_prepass_pass.set_bind_group(
        //                 2,
        //                 &world_buffer.textures[batch.texture_array_index].bind_group,
        //                 &[],
        //             );

        //             z_prepass_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
        //             z_prepass_pass
        //                 .set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        //             z_prepass_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
        //         });
        //     });
        // }

        let push_constant_render_flags = {
            let mut res = PushConstantRenderFlags::empty();

            res.set(
                PushConstantRenderFlags::RenderNoDraw,
                self.render_state.render_options.render_nodraw,
            );

            res.set(
                PushConstantRenderFlags::FullBright,
                self.render_state.render_options.full_bright,
            );

            res
        };

        let world_push_constants = WorldPushConstants {
            render_flags: push_constant_render_flags,
            time: self.time,
        };

        let push_data = bytemuck::bytes_of(&world_push_constants);

        // world opaque pass
        if true {
            let opaque_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("world opaque pass descriptor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_context.render_targets.main_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &render_context.render_targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    // need to clear stencils here because skybox mask doesn't write over it
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut opaque_pass = encoder.begin_render_pass(&opaque_pass_descriptor);
            opaque_pass.set_pipeline(&render_context.world_opaque_render_pipeline);

            // there are two set_push_constants method. WTF?
            opaque_pass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, push_data);
            opaque_pass.set_bind_group(0, &render_context.camera_buffer.bind_group, &[]);

            // only draws when there is world
            self.render_state
                .world_buffer
                .iter()
                .for_each(|world_buffer| {
                    // need these bind groups here so that the binding slots are occupied
                    opaque_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);
                    opaque_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);

                    // static world
                    world_buffer.opaque.iter().for_each(|batch| {
                        self.render_state.draw_call += 1;

                        // texture array
                        opaque_pass.set_bind_group(
                            2,
                            &world_buffer.textures[batch.texture_array_index].bind_group,
                            &[],
                        );

                        opaque_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        opaque_pass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );

                        opaque_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                    });

                    // viewmodels
                    self.render_state
                        .viewmodel_buffers
                        .iter()
                        .find(|buffer| {
                            // entity_state should be available at this point
                            let viewmodel_state =
                                &self.entity_state.as_ref().unwrap().viewmodel_state;

                            buffer.name.contains(&viewmodel_state.active_viewmodel)
                                && viewmodel_state.should_draw
                        })
                        .map(|dynamic_buffer| {
                            opaque_pass.set_bind_group(
                                1,
                                &dynamic_buffer.mvp_buffer.bind_group,
                                &[],
                            );

                            dynamic_buffer.opaque.iter().for_each(|batch| {
                                self.render_state.draw_call += 1;

                                // only change texture array
                                opaque_pass.set_bind_group(
                                    2,
                                    &dynamic_buffer.textures[batch.texture_array_index].bind_group,
                                    &[],
                                );

                                opaque_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                                opaque_pass.set_index_buffer(
                                    batch.index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint32,
                                );

                                opaque_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                            });
                        });

                    // player models
                    // TODO proper instanced drawing instead of this shit
                    self.entity_state.as_mut().map(|entity_state| {
                        entity_state
                            .playermodel_state
                            .players
                            .iter_mut()
                            // only draw active players
                            .filter(|player| player.should_draw)
                            .for_each(|player| {
                                self.render_state
                                    .playermodel_buffers
                                    .iter_mut()
                                    .find(|buffer| buffer.name.contains(&player.model_name))
                                    .map(|dynamic_buffer| {
                                        // build mvp for every instances
                                        // becuse we don't have instanced drawing, so this is done like that
                                        // haha
                                        // TODO dont do this
                                        player.mvp_buffer.update_mvp_buffer_many(
                                            player.build_mvp(&mut dynamic_buffer.transformations),
                                            0,
                                        );

                                        // use the player instance mvp buffer
                                        opaque_pass.set_bind_group(
                                            1,
                                            &player.mvp_buffer.bind_group,
                                            &[],
                                        );

                                        dynamic_buffer.opaque.iter().for_each(|batch| {
                                            self.render_state.draw_call += 1;

                                            // only change texture array
                                            opaque_pass.set_bind_group(
                                                2,
                                                &dynamic_buffer.textures[batch.texture_array_index]
                                                    .bind_group,
                                                &[],
                                            );

                                            opaque_pass.set_vertex_buffer(
                                                0,
                                                batch.vertex_buffer.slice(..),
                                            );
                                            opaque_pass.set_index_buffer(
                                                batch.index_buffer.slice(..),
                                                wgpu::IndexFormat::Uint32,
                                            );

                                            opaque_pass.draw_indexed(
                                                0..batch.index_count as u32,
                                                0,
                                                0..1,
                                            );
                                        });
                                    });
                            })
                    });
                });
        }

        // skybox mask
        if self.render_state.render_options.render_skybox {
            self.render_state
                .world_buffer
                .iter()
                .for_each(|world_buffer| {
                    let Some(batch_idx) = world_buffer.skybrush_batch_index else {
                        return;
                    };
                    let batch = &world_buffer.opaque[batch_idx];

                    let skybox_mask_pass_descriptor = wgpu::RenderPassDescriptor {
                        label: Some("world skybox mask pass descriptor"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &render_context.render_targets.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: Some(wgpu::Operations {
                                // even though this step has "Clear", it can't clear stencil
                                // need to clear stencil in skybox pass step
                                load: wgpu::LoadOp::Clear(0),
                                store: wgpu::StoreOp::Store,
                            }),
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    };

                    let mut rpass = encoder.begin_render_pass(&skybox_mask_pass_descriptor);

                    // VERY IMPORTANT
                    rpass.set_stencil_reference(1);

                    rpass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);
                    rpass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);

                    rpass.set_pipeline(&render_context.world_skybox_mask_render_pipeline);
                    rpass.set_bind_group(0, &render_context.camera_buffer.bind_group, &[]);

                    rpass.set_bind_group(
                        2,
                        &world_buffer.textures[batch.texture_array_index].bind_group,
                        &[],
                    );

                    rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                });
        }

        // skybox pass
        if self.render_state.render_options.render_skybox {
            if let Some(ref skybox_buffer) = self.render_state.skybox {
                let skybox_pass_descriptor = wgpu::RenderPassDescriptor {
                    label: Some("skybox pass descriptor"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &render_context.render_targets.main_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // load previously written opaque
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &render_context.render_targets.depth_view,
                        depth_ops: None,
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                };

                let mut rpass = encoder.begin_render_pass(&skybox_pass_descriptor);

                rpass.set_bind_group(0, &render_context.camera_buffer.bind_group, &[]);
                rpass.set_bind_group(1, &skybox_buffer.bind_group, &[]);

                rpass.set_pipeline(&render_context.skybox_loader.pipeline);
                // VERY IMPORTANT
                rpass.set_stencil_reference(1);

                rpass.set_vertex_buffer(0, skybox_buffer.vertex_buffer.slice(..));
                rpass.set_index_buffer(
                    skybox_buffer.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..skybox_buffer.index_count, 0, 0..1);
            }
        }

        // world transparent pass
        // if resolve pass runs but this pass does not, the result image is black
        // UPDATE, fake news, can skip this and resolve
        if self.render_state.render_options.render_transparent {
            let transparent_pass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("world transparent pass descriptor"),
                color_attachments: &render_context.oit_resolver.render_pass_color_attachments(),
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &render_context.render_targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut transparent_pass = encoder.begin_render_pass(&transparent_pass_descriptor);

            // transparent pass uses push constants as well
            transparent_pass.set_pipeline(&render_context.world_transparent_render_pipeline);
            transparent_pass.set_push_constants(wgpu::ShaderStages::FRAGMENT, 0, push_data);

            transparent_pass.set_bind_group(0, &render_context.camera_buffer.bind_group, &[]);

            self.render_state
                .world_buffer
                .iter()
                .for_each(|world_buffer| {
                    transparent_pass.set_bind_group(1, &world_buffer.mvp_buffer.bind_group, &[]);
                    transparent_pass.set_bind_group(3, &world_buffer.bsp_lightmap.bind_group, &[]);

                    world_buffer.transparent.iter().for_each(|batch| {
                        self.render_state.draw_call += 1;

                        // texture array
                        transparent_pass.set_bind_group(
                            2,
                            &world_buffer.textures[batch.texture_array_index].bind_group,
                            &[],
                        );

                        transparent_pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                        transparent_pass.set_index_buffer(
                            batch.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );

                        transparent_pass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                    });
                });
        }

        // oit resolve
        if self.render_state.render_options.render_transparent {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("oit resolve pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_context.render_targets.main_view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    resolve_target: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_context.oit_resolver.composite(&mut rpass);
        }

        // post processing
        // must be enabled because finalize is sending composite to swapchain
        // composite is empty at this moment
        {
            render_context
                .post_processing
                .read()
                .unwrap()
                .run_post_processing_effects(
                    &render_context.device,
                    encoder,
                    &render_context.render_targets.main_texture,
                    &render_context.render_targets.composite_texture,
                );
        }

        // writes to surface view because simply blitting doesn's work
        // surface texture does not have COPY_DST
        {
            render_context
                .finalize_render_pipeline
                .finalize_to_swapchain(encoder, &swapchain_view);
        }
    }
}
