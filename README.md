# kdr - Khang's Demo Renderer

Currently work in progress, check back later

## Progresses - Subjected to changes and addition but not subtraction.

- Renderer
  - BSP
    - [X] Face
    - [X] Texture
    - [X] Lightmap
    - [ ] Weird math lightmap in the case of older compiled HL maps
    - [ ] Light styles (unplanned)
    - Transparency
      - [X] Alpha test ("rendermode" = 4)
      - [X] Beter alpha test
      - "rendermode"
        - [X] general entity brush transparency
        - [ ] very correct transparency for according rendermode
      - [X] "renderamt"
    - [X] Named entities. Some entities aren't properly displaced
    - [X] Samey shader as the game
    - [X] "rendermode"
  - MDL
    - [X] Face
    - [X] Texture
    - [X] Model view projection
    - [ ] Bone based vertex transformation
    - [X] Shading
  - [ ] Skybox
  - [ ] SPRites
  - Texture Filtering
    - [X] Linear. Comes with the graphic API. Looks bad.
    - [ ] Bilinear. Heh?
    - [X] Nearest neighbor with AA. Looks pretty good.
    - [X] Fast cubic.
    - [ ] Hotswap between texture filtering.
  - Optimization
    - [X] Lightmap atlas
    - [X] Batch rendering based on texture
    - [X] Array of texture
    - Transparency sorting. Order independent transparency. Need to get WBOIT at the very least.
      - [X] Implemented WBOIT
    - [ ] Visibility. At the moment it renders everything. Or does it? It does render everything when there is some PVS tricks map like kz_hb_hopez or arte_drift.
    - [X] Mipmapping
  - [X] "FBO" rendering. There is a render surface and a swapchain surface.
  - Post Processing (for fun)
    - [ ] Bloom
    - [ ] HDR
    - [ ] Anisotropic Kuwahara Filtering
- Navigation
  - [X] Noclip movement
  - [X] Pitch and Yaw
  - [ ] GoldSrc movement (unplanned)
  - [ ] Mouse view
- Demo Player
  - [ ] Demo. Easy to do because this is the same code I have in other two projects
  - [ ] Ghost. Same thing. Very easy to implement
- Demo Renderer
  - [X] Framebuffer
  - [ ] Remux
  - [ ] Gstream or ffmpeg built-in. Gstream if everything contained but ffmpeg external binary seems nicer to work with.
- [X] BSP viewer. It is implicitly one.
- Integration
  - [X] Native with Vulkan
  - [ ] Web with WebGPU. Probably working just fine. Just need to write some HTML
  - BSP Loads:
    - [X] BSP self
    - [ ] BSP another
    - [X] MDL
    - [ ] SPR
  - [ ] User interface. `egui` probably works
